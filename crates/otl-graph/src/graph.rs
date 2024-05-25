use allocative::Allocative;
use otl_data::client_commands::{client_command::ClientCommands, *};
use static_interner::Intern;

use derive_more::Display;
use dice::{
    CancellationContext, DetectCycles, Dice, DiceComputations, DiceError, DiceTransaction,
    DiceTransactionUpdater, Key, UserComputationData,
};
use dupe::Dupe;
use futures::{
    future::{self, BoxFuture},
    Future, TryFutureExt,
};

use otl_events::{
    self,
    runtime_support::{GetTraceId, GetTxChannel, SetOtlRoot, SetTraceId, SetTxChannel},
    ClientCommandBundle, Event,
};

use dice::InjectedKey;
use futures::FutureExt;
use std::{fmt::Display, path::PathBuf, str::FromStr, sync::Arc};
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};

use crate::{
    commands::{Command, TargetType},
    executor::{DockerExecutor, Executor, GetExecutor, LocalExecutorBuilder, SetExecutor},
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

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Debug, Display, Allocative)]
pub struct LookupFileMaker(Arc<String>);

impl LookupCommand {
    fn from_str_ref(strref: &str) -> Self {
        Self(Arc::new(strref.to_string()))
    }
}

impl LookupFileMaker {
    fn from_str_ref(strref: &str) -> Self {
        Self(Arc::new(strref.to_string()))
    }
}

impl From<LookupCommand> for OtlErr {
    fn from(lup: LookupCommand) -> OtlErr {
        OtlErr::MissingCommandDependency {
            missing_dep_name: lup.0.to_string(),
        }
    }
}

impl From<LookupFileMaker> for OtlErr {
    fn from(lup: LookupFileMaker) -> OtlErr {
        OtlErr::MissingFileDependency {
            missing_file_name: lup.0.to_string(),
        }
    }
}

#[async_trait]
impl Key for LookupCommand {
    type Value = Result<CommandRef, LookupCommand>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Err(self.clone())
    }

    //TODO: set this
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        false
    }
}

#[async_trait]
impl Key for LookupFileMaker {
    type Value = Result<CommandRef, LookupFileMaker>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Err(self.clone())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        false
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
        let req_files = self.0.dependent_files.as_slice();
        let (command_deps, file_command_deps) = get_command_deps(ctx, deps, req_files).await;

        let all_deps: Vec<CommandRef> = command_deps
            .into_iter()
            .chain(file_command_deps.into_iter())
            .collect::<Result<Vec<CommandRef>, OtlErr>>()?;

        let futs = ctx.compute_many(all_deps.into_iter().map(|val| {
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

        let executor = ctx.global_data().get_executor();
        let _local_tx = tx.clone();

        let output = executor
            .execute_commands(
                self.0.clone(),
                ctx.per_transaction_data(),
                ctx.global_data(),
            )
            .await
            .map(|val| val.command_output().expect("We couldnt execute"));

        let output = output.map_err(|err| Arc::new(OtlErr::ExecutorFailed(err.to_string())))?;

        Ok(CommandVal {
            output,
            command: self.clone(),
        })
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        false
    }
}

async fn get_command_deps(
    ctx: &mut DiceComputations<'_>,
    dep_target_names: &[String],
    dep_file_names: &[String],
) -> (
    Vec<Result<CommandRef, OtlErr>>,
    Vec<Result<CommandRef, OtlErr>>,
) {
    fn flatten_res<S: Into<OtlErr>>(
        res: Result<Result<CommandRef, S>, DiceError>,
    ) -> Result<CommandRef, OtlErr> {
        match res {
            Ok(Ok(val)) => Ok(val),
            Ok(Err(err)) => Err(err.into()),
            Err(dice_err) => Err(OtlErr::DiceFail(dice_err)),
        }
    }
    let target_deps = ctx.compute_many(dep_target_names.iter().map(|val| {
        DiceComputations::declare_closure(
            move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandRef, OtlErr>> {
                let val = LookupCommand::from_str_ref(val);
                ctx.compute(&val).map(|val| flatten_res(val)).boxed()
            },
        )
    }));

    let comm_deps = future::join_all(target_deps).await;

    let filedeps = ctx.compute_many(dep_file_names.iter().map(|val| {
        DiceComputations::declare_closure(
            move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandRef, OtlErr>> {
                let val = LookupFileMaker::from_str_ref(val);
                ctx.compute(&val).map(|res| flatten_res(res)).boxed()
            },
        )
    }));

    let file_deps = future::join_all(filedeps).await;

    (comm_deps, file_deps)
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
        for file in command.0.outputs.iter() {
            let file_maker = LookupFileMaker(Arc::new(file.clone()));
            self.changed_to(vec![(file_maker, Ok(command.clone()))])?;
        }
        self.changed_to(vec![(lookup, Ok(command))])?;
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

/// Struct that holds all the state for executing tasks
pub struct CommandGraph {
    /// Dice is the graph library we use to figure out dependencies and pass state to events
    dice: Arc<Dice>,
    /// The commands that are currently contained in the graph -- they hold no information
    /// regarding dependencies, etc
    pub(crate) all_commands: Vec<CommandRef>,
    /// The receiver for all ClientCommands -- these kick off executions of the dice graph
    rx_chan: UnboundedReceiver<ClientCommandBundle>,
    tx_chan: Sender<Event>,
}

impl CommandGraph {
    pub async fn new(
        rx_chan: UnboundedReceiver<ClientCommandBundle>,
        tx_chan: Sender<Event>,
        cfg: ConfigureOtl,
    ) -> Result<Self, OtlErr> {
        let executor: Arc<dyn Executor> = match cfg.init_executor {
            Some(exec_val) => match exec_val {
                configure_otl::InitExecutor::Local(_) => Arc::new(
                    LocalExecutorBuilder::new()
                        .build()
                        .expect("Could not create executor"),
                ),
                configure_otl::InitExecutor::Docker(docker_cfg) => Arc::new(
                    DockerExecutor::new(docker_cfg.image_name, docker_cfg.additional_mounts)
                        .expect("Could not create docker executor"),
                ),
            },
            None => Arc::new(LocalExecutorBuilder::new().build().unwrap()),
        };

        let mut dice_builder = Dice::builder();
        dice_builder.set_otl_root(PathBuf::from(cfg.otl_root));
        dice_builder.set_executor(executor);
        dice_builder.set_tx_channel(tx_chan.clone());

        let dice = dice_builder.build(DetectCycles::Enabled);

        let graph = CommandGraph {
            dice,
            rx_chan,
            all_commands: vec![],
            tx_chan,
        };

        Ok(graph)
    }

    // This should hopefully never return
    pub async fn eat_commands(&mut self) {
        loop {
            if let Some(ClientCommandBundle {
                message:
                    ClientCommand {
                        client_commands: Some(command),
                    },
                oneshot_confirmer,
            }) = self.rx_chan.recv().await
            {
                let rv = self
                    .eat_command(command)
                    .await
                    .map_err(|err| err.to_string());
                oneshot_confirmer.send(rv);
            }
        }
    }

    async fn eat_command(&mut self, command: ClientCommands) -> Result<(), OtlErr> {
        match command {
            ClientCommands::Setter(SetCommands { command_content }) => {
                let script = serde_yaml::from_str(&command_content)?;
                let res = self.set_commands(script).await?;
            }
            ClientCommands::Runone(RunOne { command_name }) => {
                self.run_one_test(command_name).await?;
            }
            ClientCommands::Runtype(RunType { typeinfo }) => {
                self.run_all_typed(typeinfo).await?;
            }
            ClientCommands::Runmany(RunMany { command_names }) => {
                self.run_many_tests(command_names).await?;
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
        self.all_commands = commands;
        let mut ctx = ctx.commit().await;
        self.validate_graph(&mut ctx)
            .await
            .map_err(|vals| OtlErr::CommandSettingFailed {
                reason: format!("{} invalid dependencies found", vals.len()),
            })?;
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
            let _out = tx.execute_commands(refs).await;

            let val = tx.global_data().get_tx_channel();
            let trace = tx.per_transaction_data().get_trace_id();
            handle_result(_out, val, trace).await;
        });
        Ok(())
    }

    pub async fn run_many_tests(&self, test_names: Vec<String>) -> Result<(), OtlErr> {
        let ctx = self.dice.updater();
        let mut tx = ctx.commit().await;
        let mut refs = Vec::new();

        for test_name in test_names {
            let val = tx.compute(&LookupCommand(Arc::new(test_name))).await??;
            refs.push(val);
        }

        let mut tx = self.start_tx().await?;

        tokio::task::spawn(async move {
            let result = tx.execute_commands(refs).await;
            let val = tx.global_data().get_tx_channel();
            let trace = tx.per_transaction_data().get_trace_id();
            handle_result(result, val, trace).await;
        });
        Ok(())
    }

    pub async fn run_one_test(&self, test_name: impl Into<String>) -> Result<(), OtlErr> {
        let mut tx = self.start_tx().await?;
        let command = tx
            .compute(&LookupCommand(Arc::new(test_name.into())))
            .await??;

        tokio::task::spawn(async move {
            let output = tx.execute_command(&command).await;
            let val = tx.global_data().get_tx_channel();
            let trace = tx.per_transaction_data().get_trace_id();
            handle_result(vec![output], val, trace).await;
        });
        Ok(())
    }

    async fn validate_graph(&self, tx: &mut DiceTransaction) -> Result<(), Vec<OtlErr>> {
        let futs = tx.compute_many(self.all_commands.iter().map(|val| {
            DiceComputations::declare_closure(move |ctx: &mut DiceComputations| {
                get_command_deps(
                    ctx,
                    val.0.dependencies.as_slice(),
                    val.0.dependent_files.as_slice(),
                )
                .boxed()
            })
        }));
        let mut err_vec = vec![];
        let all_deps: Vec<_> = future::join_all(futs).await.into_iter().collect();
        for (command_deps, file_deps) in all_deps {
            for dep in command_deps.into_iter().chain(file_deps) {
                if let Err(err) = dep {
                    err_vec.push(err)
                }
            }
        }

        if err_vec.len() != 0 {
            let txchan = tx.global_data().get_tx_channel();
            for err in err_vec.iter() {
                txchan
                    .send(Event::graph_validate_error(err.to_string()))
                    .await;
            }

            Err(err_vec)
        } else {
            Ok(())
        }
    }
}

/// Handling logic for each command that is executed
///
/// We check each command that was executed for a runtime error -- a runtime error is an error that
/// happens with the otl runtime itself e.g. if a file isn't able to be created, or a process can't
/// be spawned off that needs to be spawned off, etc
async fn handle_result(
    compute_result: Vec<Result<CommandOutput, Arc<OtlErr>>>,
    tx: Sender<Event>,
    trace: String,
) {
    for res in compute_result {
        if let Err(ref rt_err) = res {
            let _ = tx
                .send(Event::runtime_error(rt_err.to_string(), trace.clone()))
                .await;
        }
    }

    let _ = tx.send(Event::done(trace)).await;
}

/// Handle for interacting with the OtlGraph
pub struct OtlServerHandle {
    /// Channel for sending client commands -- covers stuff like running tests
    pub tx_client: UnboundedSender<ClientCommandBundle>,
    /// Channel for receiving telemetry events from an execution
    ///
    /// Events include information like when each command starts, ends, is cancelled, etc
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

    fn testing_cfg() -> ConfigureOtl {
        ConfigureOtl {
            otl_root: std::env!("CARGO_MANIFEST_DIR").to_string(),
            job_slots: 1,
            init_executor: Some(configure_otl::InitExecutor::Local(CfgLocal {})),
        }
    }

    async fn execute_all_tests_in_file(yaml_data: &str) {
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data);

        let _script = script.unwrap();
        let (_tx, rx) = unbounded_channel();
        let (tx, rx_handle) = channel(100);

        let graph = CommandGraph::new(rx, tx, testing_cfg()).await.unwrap();
        let mut gh = TestGraphHandle { rx_chan: rx_handle };
        graph.run_all_typed("test".to_string()).await.unwrap();
        let events = gh.async_blocking_events().await;
        for event in events {
            if let otl_data::event::Et::Command(val) = event.et.unwrap() {
                if let Some(passed) = val.passed() {
                    assert!(passed)
                }
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
