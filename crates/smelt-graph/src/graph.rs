use allocative::Allocative;
use smelt_data::{
    client_commands::{client_command::ClientCommands, client_resp::ClientResponses, *},
    executed_tests::ExecutedTestResult,
};

use derive_more::Display;
use dice::{
    CancellationContext, DetectCycles, Dice, DiceComputations, DiceError, DiceTransaction,
    DiceTransactionUpdater, Key, UserComputationData,
};
use dupe::Dupe;
use futures::future::{self, BoxFuture};

use smelt_events::{
    self,
    runtime_support::{
        GetSmeltCfg, GetTraceId, GetTxChannel, SetSmeltCfg, SetTraceId, SetTxChannel,
    },
    ClientCommandBundle, Event,
};

use futures::FutureExt;
use std::{collections::HashSet, str::FromStr, sync::Arc};
use tokio::sync::mpsc::{Sender, UnboundedReceiver, UnboundedSender};

use crate::{
    commands::{Command, TargetType},
    executor::{DockerExecutor, Executor, GetExecutor, LocalExecutor, SetExecutor},
    utils::invoke_start_message,
    CommandDependency,
};
use async_trait::async_trait;
use smelt_core::CommandDefPath;
use smelt_core::SmeltErr;

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct CommandRef(Arc<Command>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct QueryCommandRef(Arc<Command>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct LookupCommand(Arc<String>);

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Debug, Display, Allocative)]
pub struct LookupFileMaker(Arc<CommandDefPath>);

impl LookupCommand {
    fn from_str_ref(strref: &str) -> Self {
        Self(Arc::new(strref.to_string()))
    }
}

impl LookupFileMaker {
    fn from_ref(strref: &CommandDefPath) -> Self {
        Self(Arc::new(strref.clone()))
    }
}

impl From<LookupCommand> for SmeltErr {
    fn from(lup: LookupCommand) -> SmeltErr {
        SmeltErr::MissingCommandDependency {
            missing_dep_name: lup.0.to_string(),
        }
    }
}

impl From<LookupFileMaker> for SmeltErr {
    fn from(lup: LookupFileMaker) -> SmeltErr {
        SmeltErr::MissingFileDependency {
            missing_file_name: lup.0.to_string(),
        }
    }
}

#[async_trait]
impl Key for LookupCommand {
    type Value = Result<CommandRef, LookupCommand>;
    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Err(self.clone())
    }

    //TODO: set this
    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        false
    }
}

#[async_trait]
impl Key for LookupFileMaker {
    type Value = Result<CommandRef, LookupFileMaker>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Err(self.clone())
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        false
    }
}

#[async_trait]
impl Key for CommandRef {
    type Value = Result<Arc<ExecutedTestResult>, Arc<SmeltErr>>;
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
            .collect::<Result<Vec<CommandRef>, SmeltErr>>()?;

        let futs = ctx.compute_many(all_deps.into_iter().map(|val| {
            DiceComputations::declare_closure(
                move |ctx: &mut DiceComputations| -> BoxFuture<Self::Value> {
                    ctx.compute(&val)
                        .map(|computed_val| match computed_val {
                            Ok(val) => val,
                            Err(err) => Err(Arc::new(SmeltErr::DiceFail(err))),
                        })
                        .boxed()
                },
            )
        }));

        let val: Vec<Self::Value> = future::join_all(futs).await.into_iter().collect();

        let mut exit = None;
        for val in val {
            match val {
                Ok(res) => {
                    if res.is_skipped() {
                        tracing::trace!("Dependency was skipped -- skipping {}", self.0.name);
                        exit = Some(Arc::new(ExecutedTestResult::Skipped));
                        break;
                    }

                    if res.get_retcode() != 0 {
                        tracing::trace!("Dependency was skipped -- skipping {}", self.0.name);
                        exit = Some(Arc::new(ExecutedTestResult::Skipped));
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Smelt runtime failed to execute a command with error {e}  -- skipping {}",
                        self.0.name
                    );
                    exit = Some(Arc::new(ExecutedTestResult::Skipped));
                    break;
                }
            }
        }

        let tx = ctx.per_transaction_data().get_tx_channel();
        if let Some(need_to_skip) = exit {
            let _ = tx
                .send(Event::command_skipped(
                    self.0.name.clone(),
                    ctx.per_transaction_data().get_trace_id(),
                ))
                .await;
            return Ok(need_to_skip);
        }

        //Currently, we do nothing with this. What we _should_ do is check if these guys fail --
        //specifically, if build targets fail -- this would be Bad and should cause an abort

        let executor = ctx.global_data().get_executor();

        let output = executor
            .execute_commands(
                self.0.clone(),
                ctx.per_transaction_data(),
                ctx.global_data(),
            )
            .await;

        let output = output.map_err(|err| Arc::new(SmeltErr::ExecutorFailed(err.to_string())))?;

        let tr = output.clone().to_test_result();

        let command_finished =
            Event::command_finished(tr, ctx.per_transaction_data().get_trace_id());
        let mut _handleme = tx.send(command_finished).await;

        Ok(Arc::new(output))
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        false
    }
}

async fn get_command_deps(
    ctx: &mut DiceComputations<'_>,
    dep_target_names: &[CommandDependency],
    dep_file_names: &[CommandDefPath],
) -> (
    Vec<Result<CommandRef, SmeltErr>>,
    Vec<Result<CommandRef, SmeltErr>>,
) {
    fn flatten_res<S: Into<SmeltErr>>(
        res: Result<Result<CommandRef, S>, DiceError>,
    ) -> Result<CommandRef, SmeltErr> {
        match res {
            Ok(Ok(val)) => Ok(val),
            Ok(Err(err)) => Err(err.into()),
            Err(dice_err) => Err(SmeltErr::DiceFail(dice_err)),
        }
    }
    let target_deps = ctx.compute_many(dep_target_names.iter().map(|val| {
        DiceComputations::declare_closure(
            move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandRef, SmeltErr>> {
                let val = LookupCommand::from_str_ref(val.get_command_name());
                ctx.compute(&val).map(flatten_res).boxed()
            },
        )
    }));

    let comm_deps = future::join_all(target_deps).await;

    let filedeps = ctx.compute_many(dep_file_names.iter().map(|val| {
        DiceComputations::declare_closure(
            move |ctx: &mut DiceComputations| -> BoxFuture<Result<CommandRef, SmeltErr>> {
                let val = LookupFileMaker::from_ref(val);
                ctx.compute(&val).map(flatten_res).boxed()
            },
        )
    }));

    let file_deps = future::join_all(filedeps).await;

    (comm_deps, file_deps)
}

pub trait CommandSetter {
    fn add_command(&mut self, command: CommandRef) -> Result<(), SmeltErr>;
    fn add_commands(
        &mut self,
        equations: impl IntoIterator<Item = CommandRef>,
    ) -> Result<(), SmeltErr>;
}
#[async_trait]
pub trait CommandExecutor {
    async fn execute_command(
        &mut self,
        command_name: &CommandRef,
    ) -> Result<Arc<ExecutedTestResult>, Arc<SmeltErr>>;

    async fn execute_commands(
        &mut self,
        command_name: Vec<CommandRef>,
    ) -> Vec<Result<Arc<ExecutedTestResult>, Arc<SmeltErr>>>;
}

#[async_trait]
impl CommandExecutor for DiceComputations<'_> {
    async fn execute_command(
        &mut self,
        command: &CommandRef,
    ) -> Result<Arc<ExecutedTestResult>, Arc<SmeltErr>> {
        match self.compute(command).await {
            Ok(val) => val,
            Err(dicey) => Err(Arc::new(SmeltErr::DiceFail(dicey))),
        }
    }
    async fn execute_commands(
        &mut self,
        commands: Vec<CommandRef>,
    ) -> Vec<Result<Arc<ExecutedTestResult>, Arc<SmeltErr>>> {
        let futs = self.compute_many(commands.into_iter().map(|val| {
            DiceComputations::declare_closure(
                move |ctx: &mut DiceComputations| -> BoxFuture<Result<Arc<ExecutedTestResult>, Arc<SmeltErr>>> {
                    ctx.compute(&val)
                        .map(|computed_val| match computed_val {
                            Ok(val) => val,
                            Err(err) => Err(Arc::new(SmeltErr::DiceFail(err))),
                        })
                        .boxed()
                },
            )
        }));

        future::join_all(futs).await
    }
}

impl CommandSetter for DiceTransactionUpdater {
    fn add_command(&mut self, command: CommandRef) -> Result<(), SmeltErr> {
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
    ) -> Result<(), SmeltErr> {
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
}

impl CommandGraph {
    pub async fn new(
        rx_chan: UnboundedReceiver<ClientCommandBundle>,
        cfg: ConfigureSmelt,
    ) -> Result<Self, SmeltErr> {
        let executor: Arc<dyn Executor> = match cfg.init_executor {
            Some(ref exec_val) => match exec_val {
                configure_smelt::InitExecutor::Local(_) => Arc::new(LocalExecutor {}),
                configure_smelt::InitExecutor::Docker(docker_cfg) => Arc::new(
                    DockerExecutor::new(
                        docker_cfg.image_name.clone(),
                        docker_cfg.additional_mounts.clone(),
                    )
                    .expect("Could not create docker executor"),
                ),
            },
            None => Arc::new(LocalExecutor {}),
        };

        let mut dice_builder = Dice::builder();
        dice_builder.set_smelt_cfg(cfg);
        dice_builder.set_executor(executor);

        let dice = dice_builder.build(DetectCycles::Enabled);

        let graph = CommandGraph {
            dice,
            rx_chan,
            all_commands: vec![],
        };

        tracing::trace!("Successfully made graph!");
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
                event_streamer,
            }) = self.rx_chan.recv().await
            {
                let rv = self
                    .eat_command(command, event_streamer.clone())
                    .await
                    .map_err(|err| err.to_string())
                    .map(|val| ClientResp {
                        client_responses: val,
                    });
                if let Err(ref err) = rv {
                    let _ = event_streamer
                        .send(Event::runtime_error(
                            err.clone(),
                            "ADD_TRACE_ID_HERE".to_string(),
                        ))
                        .await;
                }
                let _ = oneshot_confirmer.send(rv);
            }
        }
    }

    async fn eat_command(
        &mut self,
        command: ClientCommands,
        event_streamer: Sender<Event>,
    ) -> Result<Option<ClientResponses>, SmeltErr> {
        match command {
            ClientCommands::Setter(SetCommands { command_content }) => {
                let script = serde_yaml::from_str(&command_content)?;
                self.set_commands(script).await?;
            }
            ClientCommands::Runone(RunOne { command_name }) => {
                self.run_one_test(command_name, event_streamer).await?;
            }
            ClientCommands::Runtype(RunType { typeinfo }) => {
                self.run_all_typed(typeinfo, event_streamer).await?;
            }
            ClientCommands::Runmany(RunMany { command_names }) => {
                self.run_many_tests(command_names, event_streamer).await?;
            }
            ClientCommands::Getcfg(GetConfig {}) => {
                let rv = self.dice.updater();
                let val = rv
                    .existing_state()
                    .await
                    .global_data()
                    .get_smelt_cfg()
                    .clone();

                return Ok(Some(ClientResponses::CurrentCfg(val)));
            }
        }
        Ok(None)
    }

    pub async fn set_commands(&mut self, commands: Vec<Command>) -> Result<(), SmeltErr> {
        let mut ctx = self.dice.updater();
        #[tracing::instrument(name = "checking_names", level = "debug")]
        fn check_unique_outputs_and_names(commands: &Vec<Command>) -> Result<(), SmeltErr> {
            let mut outputfiles = HashSet::new();
            let mut cmdnames = HashSet::new();
            for command in commands.iter() {
                if cmdnames.contains(&command.name) {
                    return Err(SmeltErr::DuplicateCommandName {
                        name: command.name.clone(),
                    });
                }
                cmdnames.insert(&command.name);
                for output in command.outputs.iter() {
                    if outputfiles.contains(&output) {
                        return Err(SmeltErr::DuplicateOutput {
                            output: output.clone(),
                        });
                    }
                    outputfiles.insert(output);
                }
            }
            Ok(())
        }

        check_unique_outputs_and_names(&commands)?;

        let commands: Vec<CommandRef> = commands
            .into_iter()
            .map(|val| CommandRef(Arc::new(val)))
            .collect();
        ctx.add_commands(commands.iter().cloned())?;
        self.all_commands = commands;
        let mut ctx = ctx.commit().await;
        self.validate_graph(&mut ctx)
            .await
            .map_err(|vals| SmeltErr::CommandSettingFailed {
                reason: format!("{} invalid dependencies found", vals.len()),
            })?;
        tracing::trace!("Successfully validated graph!");
        Ok(())
    }

    async fn start_tx(&self, tx: Sender<Event>) -> Result<DiceTransaction, SmeltErr> {
        let ctx = self.dice.updater();
        let mut data = UserComputationData::new();

        data.init_trace_id();
        data.set_tx_channel(tx);
        let tx = ctx.commit_with_data(data).await;
        let val = tx.per_transaction_data().get_tx_channel();
        // todo -- handle err
        let _ = val
            .send(invoke_start_message(tx.per_transaction_data(), tx.global_data()).await)
            .await;

        Ok(tx)
    }

    pub async fn run_all_typed(
        &self,
        maybe_type: String,
        event_streamer: Sender<Event>,
    ) -> Result<(), SmeltErr> {
        let tt = TargetType::from_str(maybe_type.as_str())?;
        let tx = self.start_tx(event_streamer).await?;
        let refs = self
            .all_commands
            .iter()
            .filter(|&val| val.0.target_type == tt)
            .cloned()
            .collect();

        self.run_tests(refs, tx).await
    }

    async fn run_tests(
        &self,
        refs: Vec<CommandRef>,
        mut tx: DiceTransaction,
    ) -> Result<(), SmeltErr> {
        tokio::task::spawn(async move {
            let _out = tx.execute_commands(refs).await;
            let val = tx.per_transaction_data().get_tx_channel();
            let trace = tx.per_transaction_data().get_trace_id();

            handle_result(_out, val, trace).await;
        });
        Ok(())
    }
    pub async fn run_many_tests(
        &self,
        test_names: Vec<String>,
        event_streamer: Sender<Event>,
    ) -> Result<(), SmeltErr> {
        let mut tx = self.start_tx(event_streamer).await?;
        let mut refs = Vec::new();

        for test_name in test_names {
            let val = tx.compute(&LookupCommand(Arc::new(test_name))).await??;
            refs.push(val);
        }
        self.run_tests(refs, tx).await
    }

    pub async fn run_one_test(
        &self,
        test_name: impl Into<String>,
        event_streamer: Sender<Event>,
    ) -> Result<(), SmeltErr> {
        let mut tx = self.start_tx(event_streamer).await?;
        let command = tx
            .compute(&LookupCommand(Arc::new(test_name.into())))
            .await??;
        self.run_tests(vec![command], tx).await
    }

    async fn validate_graph(&self, tx: &mut DiceTransaction) -> Result<(), Vec<SmeltErr>> {
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

        if !err_vec.is_empty() {
            for err in err_vec.iter() {
                let sterr = err.to_string();
                tracing::info!("found err while validating graph: {sterr}");
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
/// happens with the smelt runtime itself e.g. if a file isn't able to be created, or a process can't
/// be spawned off that needs to be spawned off, etc
async fn handle_result(
    compute_result: Vec<Result<Arc<ExecutedTestResult>, Arc<SmeltErr>>>,
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

/// Handle for interacting with the SmeltGraph
pub struct SmeltServerHandle {
    /// Channel for sending client commands -- covers stuff like running tests
    pub tx_client: UnboundedSender<ClientCommandBundle>,
}

pub fn spawn_graph_server(cfg: ConfigureSmelt) -> SmeltServerHandle {
    let (tx_client, rx_client) = tokio::sync::mpsc::unbounded_channel();

    let server_handle = SmeltServerHandle { tx_client };

    use tokio::runtime::Builder;

    std::thread::spawn(move || {
        let rt = Builder::new_multi_thread()
            .worker_threads(4) // specify the number of threads here
            .enable_all()
            .build()
            .unwrap();

        //todo -- add failure handling here
        let mut graph = rt.block_on(CommandGraph::new(rx_client, cfg)).unwrap();
        rt.block_on(async move {
            // if either of these futures exit, we should head out
            tokio::select! {
                _graph = graph.eat_commands() => {}
            }
        });
    });
    server_handle
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tokio::{
        fs::File,
        io::AsyncReadExt,
        sync::mpsc::{channel, unbounded_channel, Receiver},
    };

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

    fn manifest_rel_path(path: &'static str) -> String {
        let manifest = std::env!("CARGO_MANIFEST_DIR").to_string();

        format!("{}/{}", manifest, path)
    }
    fn testing_cfg(_cmd_def_path: String) -> ConfigureSmelt {
        ConfigureSmelt {
            prof_cfg: Some(ProfilerCfg {
                prof_type: 0,
                sampling_period: 1000,
            }),
            smelt_root: std::env!("CARGO_MANIFEST_DIR").to_string(),
            test_only: false,
            job_slots: 1,
            init_executor: Some(configure_smelt::InitExecutor::Local(CfgLocal {})),
        }
    }

    async fn execute_all_tests_in_file(yaml_path: &'static str) {
        let yaml_path = manifest_rel_path(yaml_path);
        let mut yaml_data = String::new();

        let _ = File::open(Path::new(&yaml_path))
            .await
            .unwrap()
            .read_to_string(&mut yaml_data)
            .await
            .unwrap();
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data.as_str());

        let _script = script.unwrap();
        let (_tx, rx) = unbounded_channel();
        let (tx, rx_handle) = channel(100);

        let graph = CommandGraph::new(rx, testing_cfg(yaml_path)).await.unwrap();
        let mut gh = TestGraphHandle { rx_chan: rx_handle };
        graph
            .run_all_typed("test".to_string(), tx.clone())
            .await
            .unwrap();
        let events = gh.async_blocking_events().await;
        for event in events {
            if let smelt_data::event::Et::Command(val) = event.et.unwrap() {
                if let Some(passed) = val.passed() {
                    assert!(passed)
                }
            }
        }
    }

    #[tokio::test]
    async fn dependency_less_exec() {
        let yaml_path = "test_data/command_lists/cl1.yaml";

        execute_all_tests_in_file(yaml_path).await
    }

    #[tokio::test]
    async fn test_with_deps() {
        let yaml_path = "test_data/command_lists/cl2.yaml";
        execute_all_tests_in_file(yaml_path).await
    }

    #[tokio::test]
    async fn test_with_intraphase_deps() {
        let yaml_path = "test_data/command_lists/cl3.yaml";
        execute_all_tests_in_file(yaml_path).await
    }
}
