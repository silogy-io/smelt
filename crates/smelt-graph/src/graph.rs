use std::{collections::HashSet, str::FromStr, sync::Arc};

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::{
    CancellationContext, DetectCycles, Dice, DiceComputations, DiceError, DiceTransaction,
    DiceTransactionUpdater, Key, UserComputationData,
};
use dupe::Dupe;
use futures::FutureExt;
use futures::{
    future::{self, BoxFuture},
    stream::FuturesUnordered,
    StreamExt,
};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{Sender, UnboundedReceiver, UnboundedSender},
};

use smelt_core::{prepare_artifact_file, SmeltErr};
use smelt_core::{prepare_workspace, CommandDefPath};
use smelt_data::{
    client_commands::{
        cfg_slurm::SealedWorkspace, client_command::ClientCommands, client_resp::ClientResponses,
        DockerWorkspace, *,
    },
    executed_tests::ExecutedTestResult,
};
use smelt_events::{
    self,
    runtime_support::{
        GetCmdDefPath, GetSmeltCfg, GetSmeltRoot, GetTraceId, GetTxChannel, SetCmdDefPath,
        SetHostname, SetSmeltCfg, SetTraceId, SetTxChannel,
    },
    ClientCommandBundle, Event,
};

use crate::{
    commands::{Command, TargetType},
    executor::{DockerExecutor, Executor, GetExecutor, LocalExecutor, SetExecutor, SlurmExecutor},
    utils::invoke_start_message,
    CommandDependency,
};

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
    /// This is the work horse for the smelt execution engine
    ///
    /// Each command impls the Key trait, and this resolves how to execute each command, and each
    /// command's dependencies
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let test_only = ctx.global_data().get_smelt_cfg().test_only;
        let prepare_only = ctx.global_data().get_smelt_cfg().prepare_workspace;

        if test_only && !self.0.target_type.test_only_valid() {
            return Ok(Arc::new(ExecutedTestResult::Skipped));
        }

        let deps = self.0.dependencies.as_slice();
        let req_files = self.0.dependent_files.as_slice();
        let (command_deps, file_command_deps) = get_command_deps(ctx, deps, req_files).await;

        let all_deps: Vec<CommandRef> = command_deps
            .into_iter()
            .chain(file_command_deps.into_iter())
            .filter(|val| match val {
                Ok(res) => (res.0.target_type.test_only_valid() && test_only) || !test_only,
                Err(_) => true,
            })
            .collect::<Result<Vec<CommandRef>, SmeltErr>>()?;

        let tx = ctx.per_transaction_data().get_tx_channel();

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

        // Execute all the dependencies of this command
        let val: Vec<Self::Value> = future::join_all(futs).await.into_iter().collect();

        if prepare_only && self.0.target_type.test_only_valid() {
            tracing::info!("preparing!");
            let command = self.0.as_ref();
            let root = ctx.global_data().get_smelt_root();
            let cfg = ctx.global_data().get_smelt_cfg();
            let _ = prepare_workspace(command, root.clone(), command.working_dir.as_path())
                .await
                .inspect_err(|err| {
                    tracing::info!(
                        "Preparing the workspace for {:?} failed with err {:?}",
                        command.name,
                        err
                    )
                });

            let workspace_smelt_root =
                if let Some(configure_smelt::InitExecutor::Slurm(CfgSlurm {
                    sealed_workspace:
                        Some(SealedWorkspace::Dockerws(DockerWorkspace {
                            workspace_smelt_root,
                            ..
                        })),
                    ..
                })) = &cfg.init_executor
                {
                    workspace_smelt_root.to_string()
                } else {
                    root.to_string_lossy().to_string()
                };

            let command_working_dir = command.default_target_root(root)?;
            let _ = prepare_artifact_file(
                command,
                workspace_smelt_root.to_string(),
                command_working_dir.as_path(),
                ctx.per_transaction_data().get_cmd_def_path(),
            )
            .await
            .inspect_err(|err| {
                tracing::error!("Creating the artifact json file failed while creating the workspace with err {err}")
            });

            return Ok(Arc::new(ExecutedTestResult::Skipped));
        }

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

        if let Some(need_to_skip) = exit {
            let _ = tx
                .send(Event::command_skipped(
                    self.0.name.clone(),
                    ctx.per_transaction_data().get_trace_id(),
                ))
                .await;
            return Ok(need_to_skip);
        }

        let executor = ctx.global_data().get_executor();

        // At this point, the current command is effectively scheduled -- all of the dependencies
        // of this command have executed successfully, and we're about to try to execute this
        // command
        let _ = tx
            .send(Event::command_scheduled(
                self.0.name.clone(),
                ctx.per_transaction_data().get_trace_id(),
            ))
            .await;

        let output = executor
            .execute_commands(
                self.0.clone(),
                ctx.per_transaction_data(),
                ctx.global_data(),
            )
            .await;

        let output = output.map_err(|err| Arc::new(SmeltErr::ExecutorFailed(err.to_string())))?;

        let tr = output.clone().to_test_result();

        let command_finished = Event::command_finished(
            tr,
            self.0.target_type.to_string(),
            ctx.per_transaction_data().get_trace_id(),
        );
        let mut _handleme = tx.send(command_finished).await;
        if output.failed() {
            if let Some(ref failure_command) = self.0.on_failure {
                let lookup = LookupCommand::from_str_ref(failure_command.get_command_name());
                let dice_res = ctx.compute(&lookup).await;
                match dice_res {
                    Ok(actual_val) => match &actual_val {
                        Ok(inner) => {
                            let _rerun_result = ctx.compute(inner).await;
                        }
                        Err(err) => {
                            tracing::error!(
                        "Application level error when trying to access the on_failure command for {} with err {}", self.0.name, err 
                );
                        }
                    },
                    Err(err) => {
                        tracing::error!(
                            "Unexpected dice failure when trying to lookup the re-run value for {} with err {}",
                            self.0.name, err
                        );
                    }
                }
            }
        }

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

async fn drop_tx(tx: DiceTransaction) {
    let local_data = tx.per_transaction_data();
    tx.global_data()
        .get_executor()
        .drop_per_tx_state(local_data)
        .await;
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
                            Ok(ival) => ival,
                            Err(err) => Err(Arc::new(SmeltErr::DiceFail(err))),
                        })
                        .boxed()
                },
            )
        }));

        let mut rv = vec![];
        let mut unordered = FuturesUnordered::from_iter(futs);

        while let Some(result) = unordered.next().await {
            rv.push(result);
        }
        rv
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
    ///
    def_path: String,
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
                    DockerExecutor::new(docker_cfg).expect("Could not create docker executor"),
                ),
                configure_smelt::InitExecutor::Slurm(_slurm_cfg) => {
                    Arc::new(SlurmExecutor::new(&cfg).await)
                }
            },
            None => Arc::new(LocalExecutor {}),
        };

        let mut dice_builder = Dice::builder();
        // NOTE: this is only needed with the slurm executor

        dice_builder.set_smelt_cfg(cfg);
        dice_builder.set_executor(executor);

        let dice = dice_builder.build(DetectCycles::Enabled);

        let graph = CommandGraph {
            dice,
            rx_chan,
            all_commands: vec![],
            def_path: String::new(),
        };

        tracing::trace!("Successfully made graph!");
        Ok(graph)
    }

    // This should hopefully never return
    pub async fn eat_commands(&mut self) {
        use tokio::time::timeout;
        //TODO: maybe this should be configurable
        //      in practice for most end users, this shouldnt come up
        let duration = tokio::time::Duration::from_secs(1200);
        loop {
            let result = timeout(duration, self.rx_chan.recv()).await;

            if let Ok(Some(ClientCommandBundle {
                message:
                    ClientCommand {
                        client_commands: Some(command),
                    },
                oneshot_confirmer,
                event_streamer,
            })) = result
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
            } else if result.is_err() {
                tracing::warn!("We have elapsed on our timeout for new commands to come in -- exiting from the eatcommand loop");
            }
        }
    }

    async fn eat_command(
        &mut self,
        command: ClientCommands,
        event_streamer: Sender<Event>,
    ) -> Result<Option<ClientResponses>, SmeltErr> {
        match command {
            ClientCommands::Setter(SetCommands {
                command_content,
                maybe_def_path,
            }) => {
                let script = serde_yaml::from_str(&command_content)?;
                self.set_commands(script, maybe_def_path).await?;
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

    pub async fn set_commands(
        &mut self,
        commands: Vec<Command>,
        mut maybe_def_path: String,
    ) -> Result<(), SmeltErr> {
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

        if maybe_def_path.is_empty() {
            maybe_def_path = ctx
                .existing_state()
                .await
                .global_data()
                .get_smelt_root()
                .to_string_lossy()
                .to_string();
        }

        self.all_commands = commands;
        self.def_path = maybe_def_path;

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
        let executor = ctx.existing_state().await.global_data().get_executor();

        let mut data = UserComputationData::new();

        data.set_hostname();
        data.init_trace_id();
        data.set_tx_channel(tx);
        data.set_cmd_def_path(self.def_path.clone());
        executor.init_per_tx_state(&mut data).await;

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

    /// Top level function for running commands -- any commands executed should Go Here
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
            drop_tx(tx).await;
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

pub fn spawn_graph_server(cfg: ConfigureSmelt, runtime: &Runtime) -> SmeltServerHandle {
    let (tx_client, rx_client) = tokio::sync::mpsc::unbounded_channel();

    let server_handle = SmeltServerHandle { tx_client };

    runtime.spawn(async move {
        //todo -- add failure handling here
        let mut graph = CommandGraph::new(rx_client, cfg).await.unwrap();

        // if either of these futures exit, we should head out
        tokio::select! {
            _graph = graph.eat_commands() => {}
        }
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

    use crate::executor::RemoteExecutor;

    use super::*;

    impl CommandGraph {
        pub async fn new_remote(
            rx_chan: UnboundedReceiver<ClientCommandBundle>,
            cfg: ConfigureSmelt,
        ) -> Result<Self, SmeltErr> {
            let executor: Arc<dyn Executor> = Arc::new(RemoteExecutor::new().await);

            let mut dice_builder = Dice::builder();
            dice_builder.set_smelt_cfg(cfg);
            dice_builder.set_executor(executor);

            let dice = dice_builder.build(DetectCycles::Enabled);

            let graph = CommandGraph {
                dice,
                rx_chan,
                all_commands: vec![],
                def_path: String::new(),
            };

            tracing::trace!("Successfully made graph!");
            Ok(graph)
        }
    }

    struct TestGraphHandle {
        rx_chan: Receiver<Event>,
    }

    impl TestGraphHandle {
        pub async fn async_blocking_events(&mut self) -> Vec<Event> {
            let mut rv = vec![];
            loop {
                if let Some(val) = self.rx_chan.recv().await {
                    rv.push(val.clone());
                    if val.finished_event() {
                        break;
                    }
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
            prepare_workspace: false,
            smelt_root: std::env!("CARGO_MANIFEST_DIR").to_string(),
            test_only: false,
            silent: true,
            sandbox_env: false,
            job_slots: 1,
            init_executor: Some(configure_smelt::InitExecutor::Local(CfgLocal {})),
        }
    }

    async fn local_execute_tests(yaml_path: &'static str) {
        let yaml_path = manifest_rel_path(yaml_path);
        let (_tx, rx) = unbounded_channel();
        let graph = CommandGraph::new(rx, testing_cfg(yaml_path.clone()))
            .await
            .unwrap();
        execute_all_tests_in_file(graph, yaml_path).await
    }

    async fn remote_execute_tests(yaml_path: &'static str) {
        let yaml_path = manifest_rel_path(yaml_path);
        let (_tx, rx) = unbounded_channel();
        let graph = CommandGraph::new_remote(rx, testing_cfg(yaml_path.clone()))
            .await
            .unwrap();
        execute_all_tests_in_file(graph, yaml_path).await
    }

    async fn execute_all_tests_in_file(mut graph: CommandGraph, yaml_path: String) {
        let mut yaml_data = String::new();

        let _ = File::open(Path::new(&yaml_path))
            .await
            .unwrap()
            .read_to_string(&mut yaml_data)
            .await
            .unwrap();
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data.as_str());

        let _script = script.unwrap();
        graph
            .set_commands(_script, "".to_string())
            .await
            .expect("Setting commands failed!");

        let (tx, rx_handle) = channel(100);

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

        local_execute_tests(yaml_path).await
    }

    //#[tokio::test]
    //async fn dependency_less_exec_remote() {
    //    let yaml_path = "test_data/command_lists/cl1.yaml";

    //    remote_execute_tests(yaml_path).await
    //}

    #[tokio::test]
    async fn test_with_deps() {
        let yaml_path = "test_data/command_lists/cl2.yaml";
        local_execute_tests(yaml_path).await
    }

    #[tokio::test]
    async fn test_with_intraphase_deps() {
        let yaml_path = "test_data/command_lists/cl3.yaml";
        local_execute_tests(yaml_path).await
    }
}
