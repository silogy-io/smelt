use allocative::Allocative;
use otl_data::client_commands::{client_command::ClientCommands, *};

use derive_more::Display;
use dice::{
    CancellationContext, DetectCycles, Dice, DiceComputations, DiceTransaction,
    DiceTransactionUpdater, Key, UserComputationData,
};
use dupe::Dupe;
use futures::{
    future::{self, BoxFuture},
    Future, StreamExt, TryFutureExt,
};
use otl_data::CommandStarted;
use otl_events::{
    self, new_command_event,
    runtime_support::{GetTraceId, GetTxChannel, SetTraceId, SetTxChannel},
    Event,
};

use dice::InjectedKey;
use futures::FutureExt;
use std::{str::FromStr, sync::Arc};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender},
};

use crate::{
    commands::{Command, TargetType},
    executor::{Executor, GetExecutor, LocalExecutorBuilder, SetExecutor},
    utils::invoke_start_message,
};
use async_trait::async_trait;
use otl_core::OtlErr;
use otl_data::CommandOutput;

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct CommandRef(Arc<Command>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct CommandVal {
    output: CommandOutput,
    command: CommandRef,
}

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct QueryCommandRef(Arc<Command>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct LookupCommand(Arc<String>);

impl From<QueryCommandRef> for CommandRef {
    fn from(value: QueryCommandRef) -> Self {
        Self(value.0)
    }
}

impl From<CommandRef> for QueryCommandRef {
    fn from(value: CommandRef) -> Self {
        Self(value.0)
    }
}

impl LookupCommand {
    fn from_str_ref(strref: &String) -> Self {
        Self(Arc::new(strref.clone()))
    }
}

impl InjectedKey for LookupCommand {
    type Value = CommandRef;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for CommandRef {
    type Value = Result<CommandVal, Arc<OtlErr>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let deps = self.0.dependencies.as_slice();
        let command_deps = get_command_deps(ctx, deps).await?;

        let futs = ctx.compute_many(command_deps.into_iter().map(|val| {
            DiceComputations::declare_closure(
                move |ctx: &mut DiceComputations| -> BoxFuture<Self::Value> {
                    ctx.compute(&val)
                        .map(|computed_val| match computed_val {
                            Ok(val) => val,
                            Err(err) => Err(Arc::new(OtlErr::DiceFail(err))),
                        })
                        .boxed()
                },
            )
        }));

        let _val: Vec<Self::Value> = future::join_all(futs).await.into_iter().collect();
        //Currently, we do nothing with this. What we _should_ do is check if these guys fail --
        //specifically, if build targets fail -- this would be Bad and should cause an abort
        let tx = ctx.global_data().get_tx_channel();
        let name = self.0.name.clone();
        let trace = ctx.per_transaction_data().get_trace_id();

        let _ = tx
            .send(new_command_event(
                name.clone(),
                otl_data::command_event::CommandVariant::Started(CommandStarted {}),
                trace,
            ))
            .await;
        let executor = ctx.global_data().get_executor();
        let local_tx = tx.clone();

        let _recv: Vec<CommandOutput> = executor
            .command_as_stream(self.0.clone())
            .await
            .unwrap()
            .filter_map(|val| {
                println!("heyy");
                let local_tx_clone = local_tx.clone();
                async move {
                    let _ = local_tx_clone.send(val.clone()).await;
                    val.command_output()
                }
            })
            .collect()
            .await;

        //if recv.len() != 1 {
        //    panic!("Todo -- handle this, we should only see one command output message -- we should be able to fail more gracefully than this");
        //}
        //let output = recv.first().cloned().unwrap();

        Ok(CommandVal {
            output: CommandOutput { status_code: 0 },
            command: self.clone(),
        })
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        false
    }
}

async fn get_command_deps(
    ctx: &mut DiceComputations<'_>,
    var: &[String],
) -> Result<Vec<CommandRef>, OtlErr> {
    let futs = ctx.compute_many(var.iter().map(|val| {
        DiceComputations::declare_closure(
            move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandRef, OtlErr>> {
                let val = LookupCommand::from_str_ref(val);
                ctx.compute(&val).map_err(OtlErr::DiceFail).boxed()
            },
        )
    }));

    future::join_all(futs).await.into_iter().collect()
}

pub trait CommandSetter {
    fn add_command(&mut self, command: CommandRef) -> Result<(), OtlErr>;
    fn add_commands(
        &mut self,
        equations: impl IntoIterator<Item = CommandRef>,
    ) -> Result<(), OtlErr>;
}
pub type CommandOutputFuture<'a> =
    dyn Future<Output = Result<CommandOutput, Arc<OtlErr>>> + Send + 'a;
#[async_trait]
pub trait CommandExecutor {
    async fn execute_command(
        &mut self,
        command_name: &CommandRef,
    ) -> Result<CommandOutput, Arc<OtlErr>>;

    async fn execute_commands(
        &mut self,
        command_name: Vec<CommandRef>,
    ) -> Vec<Result<CommandOutput, Arc<OtlErr>>>;
}

#[async_trait]
impl CommandExecutor for DiceComputations<'_> {
    async fn execute_command(
        &mut self,
        command: &CommandRef,
    ) -> Result<CommandOutput, Arc<OtlErr>> {
        match self.compute(command).await {
            Ok(val) => val.map(|val| val.output),
            Err(dicey) => Err(Arc::new(OtlErr::DiceFail(dicey))),
        }
    }
    async fn execute_commands(
        &mut self,
        commands: Vec<CommandRef>,
    ) -> Vec<Result<CommandOutput, Arc<OtlErr>>> {
        let futs = self.compute_many(commands.into_iter().map(|val| {
            DiceComputations::declare_closure(
                move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandOutput, Arc<OtlErr>>> {
                    ctx.compute(&val)
                        .map(|computed_val| match computed_val {
                            Ok(val) => val.map(|val| val.output),
                            Err(err) => Err(Arc::new(OtlErr::DiceFail(err))),
                        })
                        .boxed()
                },
            )
        }));

        future::join_all(futs).await
    }
}

impl CommandSetter for DiceTransactionUpdater {
    fn add_command(&mut self, command: CommandRef) -> Result<(), OtlErr> {
        let lookup = LookupCommand::from_str_ref(&command.0.name);

        self.changed_to(vec![(lookup, command)])?;
        Ok(())
    }

    fn add_commands(
        &mut self,
        commands: impl IntoIterator<Item = CommandRef>,
    ) -> Result<(), OtlErr> {
        for command in commands {
            self.add_command(command)?;
        }
        Ok(())
    }
}

pub struct CommandGraph {
    dice: Arc<Dice>,
    pub(crate) all_commands: Vec<CommandRef>,
    rx_chan: UnboundedReceiver<ClientCommand>,
}

impl CommandGraph {
    pub async fn new(
        rx_chan: UnboundedReceiver<ClientCommand>,
        tx_chan: Sender<Event>,
    ) -> Result<Self, OtlErr> {
        let executor = LocalExecutorBuilder::new().threads(8).build()?;

        let mut dice_builder = Dice::builder();
        dice_builder.set_executor(executor);
        dice_builder.set_tx_channel(tx_chan);

        let dice = dice_builder.build(DetectCycles::Enabled);

        let graph = CommandGraph {
            dice,
            rx_chan,
            all_commands: vec![],
        };

        Ok(graph)
    }

    // This should hopefully never return
    pub async fn eat_commands(&mut self) {
        loop {
            if let Some(ClientCommand {
                client_commands: Some(command),
            }) = self.rx_chan.recv().await
            {
                let rv = self.eat_command(command).await;
                if let Err(err) = rv {
                    dbg!("Todo -- send out a warning via a channel or something, at least log with tracing at this point when i add it in");
                }
            }
        }
    }

    async fn eat_command(&mut self, command: ClientCommands) -> Result<(), OtlErr> {
        match command {
            ClientCommands::Setter(SetCommands { command_content }) => {
                let script = serde_yaml::from_str(&command_content)?;
                self.set_commands(script).await?;
            }
            ClientCommands::Runone(RunOne { command_name }) => {
                self.run_one_test(command_name).await?;
            }
            ClientCommands::Runtype(RunType { typeinfo }) => {
                self.run_all_typed(typeinfo).await?;
            }
        }
        Ok(())
    }

    pub async fn set_commands(&mut self, commands: Vec<Command>) -> Result<(), OtlErr> {
        let mut ctx = self.dice.updater();
        let commands: Vec<CommandRef> = commands
            .into_iter()
            .map(|val| CommandRef(Arc::new(val)))
            .collect();
        ctx.add_commands(commands.iter().cloned())?;

        let _ctx = ctx.commit().await;
        Ok(())
    }

    async fn start_tx(&self) -> Result<DiceTransaction, OtlErr> {
        let ctx = self.dice.updater();
        let mut data = UserComputationData::new();

        data.init_trace_id();

        let tx = ctx.commit_with_data(data).await;
        let val = tx.global_data().get_tx_channel();
        // todo -- handle err
        let _ = val
            .send(invoke_start_message(
                tx.per_transaction_data().get_trace_id(),
            ))
            .await;

        Ok(tx)
    }

    pub async fn run_all_typed(&self, maybe_type: String) -> Result<(), OtlErr> {
        let tt = TargetType::from_str(maybe_type.as_str())?;
        let refs = self
            .all_commands
            .iter()
            .filter(|&val| val.0.target_type == tt)
            .cloned()
            .collect();

        let mut tx = self.start_tx().await?;

        tokio::task::spawn(async move {
            tx.execute_commands(refs).await;
            let val = tx.global_data().get_tx_channel();
            let trace = tx.per_transaction_data().get_trace_id();
            let _ = val.send(Event::done(trace)).await;
        });
        Ok(())
    }

    pub async fn run_one_test(&self, test_name: impl Into<String>) -> Result<(), OtlErr> {
        let ctx = self.dice.updater();
        let mut tx = ctx.commit().await;
        let command = tx
            .compute(&LookupCommand(Arc::new(test_name.into())))
            .await?;

        //tx.execute_command(&command).await

        let mut tx = self.start_tx().await?;

        tokio::task::spawn(async move {
            let _ = tx.execute_command(&command).await;
            let val = tx.global_data().get_tx_channel();
            let trace = tx.per_transaction_data().get_trace_id();
            val.send(Event::done(trace)).await
        });
        Ok(())
    }
}

pub fn spawn_otl_server() -> OtlServerHandle {
    let (tx_client, rx_client) = tokio::sync::mpsc::unbounded_channel();
    let (tx_tele, rx_tele) = tokio::sync::mpsc::channel(100);

    use tokio::runtime::Builder;

    std::thread::spawn(|| {
        let rt = Builder::new_multi_thread()
            .worker_threads(4) // specify the number of threads here
            .enable_all()
            .build()
            .unwrap();

        //todo -- add failure handling here
        let mut graph = rt.block_on(CommandGraph::new(rx_client, tx_tele)).unwrap();
        rt.block_on(graph.eat_commands());
    });
    OtlServerHandle { rx_tele, tx_client }
}

pub struct OtlServerHandle {
    pub tx_client: UnboundedSender<ClientCommand>,
    pub rx_tele: Receiver<Event>,
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::{channel, unbounded_channel};

    use super::*;

    struct TestGraphHandle {
        rx_chan: Receiver<Event>,
    }

    impl TestGraphHandle {
        pub async fn async_blocking_events(&mut self) -> Vec<Event> {
            let mut rv = vec![];
            loop {
                if let Some(val) = self.rx_chan.recv().await {
                    if val.finished_event() {
                        break;
                    }
                    rv.push(val);
                }
            }
            rv
        }
    }

    async fn execute_all_tests_in_file(yaml_data: &str) {
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data);

        let script = script.unwrap();
        let (tx, rx) = unbounded_channel();
        let (tx, rx_handle) = channel(100);
        let graph = CommandGraph::new(rx, tx).await.unwrap();
        let mut gh = TestGraphHandle { rx_chan: rx_handle };
        graph.run_all_typed("test".to_string()).await.unwrap();
        let events = gh.async_blocking_events().await;
        for event in events {
            match event.et.unwrap() {
                otl_data::event::Et::Command(val) => {
                    if let Some(passed) = val.passed() {
                        dbg!(val);
                        assert!(passed)
                    }
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn dependency_less_exec() {
        let yaml_data = include_str!("../../../test_data/command_lists/cl1.yaml");

        execute_all_tests_in_file(yaml_data).await
    }

    #[tokio::test]
    async fn test_with_deps() {
        let yaml_data = include_str!("../../../test_data/command_lists/cl2.yaml");
        execute_all_tests_in_file(yaml_data).await
    }

    #[tokio::test]
    async fn test_with_intraphase_deps() {
        let yaml_data = include_str!("../../../test_data/command_lists/cl3.yaml");
        execute_all_tests_in_file(yaml_data).await
    }
}
