use std::{
    fs::set_permissions,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{LazyLock, RwLock},
};
use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use dice::{DiceData, UserComputationData};
use scc::HashMap;
use smelt_core::{get_target_root, SmeltErr};

use std::io::Write;

use tokio::{fs::File, io::AsyncWriteExt, net::TcpListener};

use tokio::sync::{mpsc::Sender, oneshot};
use tonic::{transport::Server, Response};

use smelt_data::{
    client_commands::{
        cfg_slurm::SealedWorkspace, configure_smelt::InitExecutor, CfgSlurm, ConfigureSmelt,
        DockerWorkspace,
    },
    executed_tests::{ExecutedTestResult, TestResult},
    Event, TaggedResult,
};
use smelt_events::runtime_support::{
    GetHostname, GetSmeltCfg, GetSmeltRoot, GetTraceId, GetTxChannel,
};

use crate::executor::Executor;
use crate::Command;

use super::common::create_test_result;

fn sbatch_file() -> &'static str {
    "sbatch_command.sh"
}

struct SlurmWorkspace {
    sbatch_file: PathBuf,
}

#[derive(Debug)]
struct ProxyState {
    servers: ServerMap,
    jh: std::thread::JoinHandle<()>,
    port: u16,
}
const MAYBE_PROXY: LazyLock<Arc<RwLock<Option<ProxyState>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(None)));
type ServerMap = Arc<HashMap<String, RemoteServer>>;

pub fn init_proxy(port: u16) -> u16 {
    let innited_port = {
        let binding = MAYBE_PROXY.clone();
        let val = binding.read().unwrap().as_ref().map(|val| val.port.clone());
        val
    };
    if let Some(port) = innited_port {
        tracing::info!("Previously initialized server -- we are just returning the port");
        port
    } else {
        let servers = Arc::new(HashMap::new());
        let srv = GlobalSlurmServer {
            all_live_traces: servers.clone(),
        };

        let listener = std::net::TcpListener::bind(format!("0.0.0.0:{port}")).unwrap();
        listener
            .set_nonblocking(true)
            .expect("Cannot set nonblocking");
        let bound_port = listener.local_addr().expect("Binding failed").port();
        tracing::info!("Already ");

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async move {
                let listener = TcpListener::from_std(listener).expect("Could not convert from std");

                Server::builder()
                    .add_service(smelt_data::event_listener_server::EventListenerServer::new(
                        srv,
                    ))
                    .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                    .await
                    .unwrap();
            });
        });

        let local = MAYBE_PROXY.clone();
        *local.write().unwrap() = Some(ProxyState {
            servers,
            jh: handle,
            port,
        });
        let local2 = MAYBE_PROXY.clone();

        tracing::info!("successfully wrote?");
        tracing::info!("reading val, the val is {:?}", local2.read());

        bound_port
    }
}

async fn insert_remote_server(trace_id: String, server: RemoteServer) -> anyhow::Result<()> {
    tracing::info!("Inserting server with trace id {trace_id}");

    let srvs = {
        let binding = MAYBE_PROXY.clone();
        let val = binding.read().unwrap();
        tracing::info!("Val is {:?}", val);
        let val2 = val.as_ref();
        if let Some(sh) = val2 {
            tracing::info!("Inserting server with trace id {trace_id}");
            sh.servers.clone()
        } else {
            anyhow::bail!("NOT INITIALIZED")
        }
    };
    let _ = srvs
        .insert_async(trace_id.clone(), server)
        .await
        .inspect_err(|e| {
            tracing::error!(
                "Could not insert remote server for trace_id: {trace_id}, failed with {e:?}"
            )
        });
    Ok(())
}

async fn remove_remote_server(trace_id: String) -> anyhow::Result<()> {
    let binding = MAYBE_PROXY.clone();
    let mut val = binding.write().unwrap();
    let val2 = val.as_mut();
    if let Some(sh) = val2 {
        tracing::info!("cleaning up state for {trace_id} in the smelt slurm server");
        sh.servers.remove(&trace_id)
    } else {
        anyhow::bail!("REMOTE SERER NOT INITIALIZED");
    };
    Ok(())
}

fn aws_awgs(cfg: &CfgSlurm) -> Option<Vec<String>> {
    cfg.creds.clone().map(|creds| {
        vec![
            "--aws-key".to_string(),
            creds.key,
            "--aws_key-id".to_string(),
            creds.key_id,
            "--aws-bucket".to_string(),
            creds.bucket,
            "--s3-key-base-path".to_string(),
            creds.key_base_path,
        ]
    })
}

fn create_slurm_command(
    command: &Command,
    smelt_root: PathBuf,
    worker_bin_path: &Path,
    trace_id: &str,
    server_addr: &str,
    ws: &CfgSlurm,
) -> Result<String, SmeltErr> {
    let working_dir = command.default_target_root(smelt_root.as_path())?;
    let maybe_aws_cli = aws_awgs(ws);

    match ws.sealed_workspace.clone().expect("We need this") {
        SealedWorkspace::None(_) => {
            let mut arrrggs = vec![
                "--command-path".to_string(),
                working_dir.to_string_lossy().to_string(),
                "--command-name".to_string(),
                command.name.clone(),
                "--trace-id".to_string(),
                trace_id.to_string(),
                "--host".to_string(),
                format!("http://{}", server_addr),
            ];

            if let Some(mut aws) = maybe_aws_cli {
                arrrggs.append(&mut aws);
            }

            Ok(format!(
                "{} {}\n",
                worker_bin_path.to_string_lossy(),
                arrrggs.join(" ")
            ))
        }
        SealedWorkspace::Dockerws(DockerWorkspace {
            container_name,
            workspace_smelt_root,
            docker_args,
        }) => {
            let sealed_working_dir =
                command.default_target_root(PathBuf::from(workspace_smelt_root))?;

            let mut arrrggs = vec![
                "--command-path".to_string(),
                sealed_working_dir.to_string_lossy().to_string(),
                "--command-name".to_string(),
                command.name.clone(),
                "--trace-id".to_string(),
                trace_id.to_string(),
                "--host".to_string(),
                format!("http://{}", server_addr),
            ];

            if let Some(mut aws) = maybe_aws_cli {
                arrrggs.append(&mut aws);
            }

            let docker_run_args = docker_args.join(" ");

            Ok(format!(
                "docker run {} {} {} {}",
                docker_run_args,
                container_name,
                WORKER_PATH,
                arrrggs.join(" ")
            ))
        }
    }
}
async fn prepare_slurm_workspace(
    command: &Command,
    smelt_root: PathBuf,
    command_working_dir: &Path,
    worker_bin_path: &Path,
    trace_id: &str,
    server_addr: &str,
    ws: &CfgSlurm,
) -> anyhow::Result<SlurmWorkspace> {
    let working_dir = command.default_target_root(smelt_root.as_path())?;
    let script_file = working_dir.join(Command::script_file());
    let sbatch_file = working_dir.join(sbatch_file());
    let stdout_file = working_dir.join(Command::stdout_file());
    tokio::fs::create_dir_all(&working_dir).await?;
    let mut file = File::create(&script_file).await?;
    let mut sbatch_file_real = File::create(&sbatch_file).await?;

    let _stdout = File::create(&stdout_file).await?;

    let mut buf: Vec<u8> = Vec::new();

    writeln!(buf, "export SMELT_ROOT={}", smelt_root.to_string_lossy())?;

    writeln!(
        buf,
        "export TARGET_ROOT={}",
        get_target_root(smelt_root.to_string_lossy(), &command.name)
    )?;

    writeln!(buf, "cd {}", command_working_dir.to_string_lossy())?;

    for script_line in &command.script {
        writeln!(buf, "{}", script_line)?;
    }

    //TODO: add sbatch directives
    let mut buf2: Vec<u8> = Vec::new();

    writeln!(buf2, "#!/bin/bash")?;

    let slurm_command = create_slurm_command(
        command,
        smelt_root.clone(),
        worker_bin_path,
        trace_id,
        server_addr,
        ws,
    )?;

    writeln!(buf2, "{}\n", slurm_command)?;
    sbatch_file_real.write_all(&buf2).await?;

    file.write_all(&buf).await?;
    file.flush().await?;
    Ok(SlurmWorkspace { sbatch_file })
}

type TRMap = Arc<HashMap<String, tokio::sync::oneshot::Sender<TestResult>>>;

/// This is a dummy executor to test all of the logic of the slurm executor, with none of the
/// overhead of creating a slurm cluster
pub struct SlurmExecutor {
    cfg: CfgSlurm,
}

struct GlobalSlurmServer {
    all_live_traces: ServerMap,
}

#[tonic::async_trait]
impl smelt_data::event_listener_server::EventListener for GlobalSlurmServer {
    async fn send_event(
        &self,
        request: tonic::Request<Event>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let inner_event = request.into_inner();

        let server = self.all_live_traces.get_async(&inner_event.trace_id).await;

        if let Some(srv) = server {
            let _resp = srv.tx_chan.send(inner_event).await;
        } else {
            tracing::warn!("Received event {inner_event:?} from unregistered trace");
        }

        Ok(Response::new(()))
    }
    async fn send_outputs(
        &self,
        request: tonic::Request<TaggedResult>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let val = request.into_inner();

        let trace = val.trace_id;
        let server = self.all_live_traces.get_async(&trace).await;
        let val = val.results.expect("No results");

        tracing::trace!("Trying to remove {}", val.test_name);
        let v = if let Some(srv) = server {
            srv.connections.remove_async(&val.test_name).await
        } else {
            None
        };
        match v {
            None => {
                tracing::error!("Missing entry in the remote server!");
            }
            Some(entry) => {
                let _ = entry.1.send(val);
            }
        };
        Ok(Response::new(()))
    }
}

#[derive(Debug, Clone)]
struct RemoteServer {
    tx_chan: Sender<Event>,
    connections: TRMap,
}

struct TestRemoteServer {}

pub const WORKER_BIN: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_SMELT_SLURM_worker"));

async fn make_temp_executable(cfg: &ConfigureSmelt, data: &[u8]) -> anyhow::Result<PathBuf> {
    let file = SlurmExecutor::get_bin(cfg);

    tokio::fs::write(file.as_path(), data).await?;
    let mut perms = tokio::fs::metadata(file.as_path()).await?.permissions();
    perms.set_mode(0o755); // make exec
    tokio::fs::set_permissions(file.as_path(), perms).await?;
    Ok(file)
}

struct PerTxRemoteState {
    connections: TRMap,
    hostname: String,
    client_port: u16,
}

impl SlurmExecutor {
    pub async fn new(global_cfg: &ConfigureSmelt) -> Self {
        let _res = make_temp_executable(global_cfg, WORKER_BIN).await.unwrap();
        if let Some(ref executor) = global_cfg.init_executor {
            match executor {
                InitExecutor::Slurm(slurm) => Self { cfg: slurm.clone() },
                _ => {
                    panic!("Trying to init a slurm executor without the slurm variant -- something is wrong with the slurm init logic!")
                }
            }
        } else {
            panic!("No executor provided -- was expecting the slurm executor");
        }
    }
    fn get_bin(cfg: &ConfigureSmelt) -> PathBuf {
        PathBuf::from(format!("{}/workerguy", cfg.smelt_root))
    }
}

#[tonic::async_trait]
impl smelt_data::event_listener_server::EventListener for TestRemoteServer {
    async fn send_event(
        &self,
        request: tonic::Request<Event>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let inner_event = request.into_inner();

        println!("inner event is {:?}", inner_event);
        Ok(Response::new(()))
    }
    async fn send_outputs(
        &self,
        request: tonic::Request<TaggedResult>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let val = request.into_inner();
        println!("result is {:?}", val);
        Ok(Response::new(()))
    }
}

trait RemoteHelpers {
    fn set_pertx_state(&mut self, pertxstate: PerTxRemoteState);
    fn get_pertx_state(&self) -> Arc<PerTxRemoteState>;
}

impl RemoteHelpers for UserComputationData {
    fn set_pertx_state(&mut self, map: PerTxRemoteState) {
        self.data.set(Arc::new(map));
    }
    fn get_pertx_state(&self) -> Arc<PerTxRemoteState> {
        self.data.get().cloned().unwrap()
    }
}

#[async_trait]
impl Executor for SlurmExecutor {
    async fn drop_per_tx_state(&self, data: &UserComputationData) {
        let _ = remove_remote_server(data.get_trace_id()).await;
    }

    async fn init_per_tx_state(&self, data: &mut UserComputationData) {
        let tx_chan = data.get_tx_channel();
        let connections = Arc::new(HashMap::new());
        let remote_server = RemoteServer {
            tx_chan,
            connections: connections.clone(),
        };

        tracing::info!("cfg info is {:?}", self.cfg.maybe_info);

        let (mut chn, port) = self
            .cfg
            .maybe_info
            .clone()
            .map(|info| (info.hostname, info.port))
            .unwrap_or_else(|| (data.get_hostname(), 0));
        if chn.is_empty() {
            chn = data.get_hostname();
        }

        let port = init_proxy(port as u16);
        let addr = format!("0:0:0:0:{port}");
        {
            let binding = MAYBE_PROXY.clone();
            let val = binding.read().unwrap();
            tracing::info!("peeking at val, is {:?}", val);
        }
        let trace = data.get_trace_id();
        tracing::info!("Trying to insert server with trace id {trace}");
        let _ = insert_remote_server(trace, remote_server)
            .await
            .inspect_err(|err| tracing::error!("Failed to init pertx server with err {err}"));

        tracing::info!("Created server with addr {addr:?}");
        tracing::info!("sending messages to {chn} ");

        let pertx = PerTxRemoteState {
            connections,
            hostname: chn,
            client_port: port,
        };
        data.set_pertx_state(pertx);
    }

    async fn execute_commands(
        &self,
        command: Arc<Command>,
        dd: &UserComputationData,
        global_data: &DiceData,
    ) -> anyhow::Result<ExecutedTestResult> {
        let _tx = dd.get_tx_channel();

        let trace_id = dd.get_trace_id();
        let root = global_data.get_smelt_root();
        let command = command.as_ref();
        let pertxstate = dd.get_pertx_state();
        let cfg = global_data.get_smelt_cfg();
        let worker_bin = Self::get_bin(cfg);
        let addr = format!("{}:{}", pertxstate.hostname, pertxstate.client_port);

        let (sender, rcv) = oneshot::channel();
        tracing::trace!("Trying to insert {}", command.name);

        let _ = pertxstate
            .connections
            .insert_async(command.name.clone(), sender)
            .await
            .expect("Command should only be inserted once");

        // TODO:  minimize unwraps
        let _sbatch_handle = match &self.cfg.sealed_workspace.clone().unwrap() {
            SealedWorkspace::None(_) => {
                let SlurmWorkspace { sbatch_file } = prepare_slurm_workspace(
                    command,
                    root.clone(),
                    command.working_dir.as_path(),
                    worker_bin.as_path(),
                    trace_id.as_str(),
                    addr.to_string().as_str(),
                    &self.cfg,
                )
                .await?;

                let mut commandlocal = tokio::process::Command::new("sbatch");
                commandlocal.arg("--output=/dev/null");
                commandlocal.arg("--error=/dev/null");

                commandlocal.arg(&sbatch_file);

                commandlocal.env_clear().spawn()?
            }
            SealedWorkspace::Dockerws(_) => {
                let command = create_slurm_command(
                    command,
                    root.clone(),
                    worker_bin.as_path(),
                    trace_id.as_str(),
                    addr.to_string().as_str(),
                    &self.cfg,
                )?;

                let mut commandlocal = tokio::process::Command::new("sbatch");
                commandlocal.arg("--output=/tmp/smelt/out.log");
                commandlocal.arg("--error=/tmp/smelt/err.log");
                commandlocal.arg(format!("--wrap={}", command));
                tracing::info!("Command executed is {command}");

                commandlocal.env_clear().spawn()?
            }
        };
        //let stderr = comm_handle.stderr.take().unwrap();
        //let stderr_reader = BufReader::new(stderr);
        //let mut stderr_lines = stderr_reader.lines();

        //let reader = BufReader::new(comm_handle.stdout.take().unwrap());
        //let mut lines = reader.lines();

        //loop {
        //    tokio::select!(
        //        Ok(Some(line)) = lines.next_line() => {
        //            tracing::trace!("stdout says {line}");
        //        }
        //        Ok(Some(line)) = stderr_lines.next_line() => {
        //            tracing::info!("stderr says {line}");
        //        }
        //        status_code = comm_handle.wait() => {
        //            let status_code = status_code.unwrap();
        //            tracing::info!("sbatch exited with {status_code}");
        //            break;
        //        }
        //    );
        //}

        tracing::info!("Waiting for the message...");
        let output = rcv.await?;

        Ok(create_test_result(
            command,
            output
                .outputs
                .map(|outs| outs.exit_code)
                .expect("Need to have an output"),
            global_data,
        ))
    }
}

pub const WORKER_PATH: &str = "/tmp/smelt/smelt-worker";

pub fn init_worker_binary() -> Result<(), std::io::Error> {
    let wpath = std::path::PathBuf::from(WORKER_PATH);
    //TODO -- maybe handle this
    let _tohandle = std::fs::create_dir_all(wpath.parent().unwrap());
    std::fs::write(WORKER_PATH, WORKER_BIN)?;
    let mut perms = std::fs::metadata(WORKER_PATH)?.permissions();
    perms.set_mode(0o777);
    set_permissions(WORKER_PATH, perms)?;
    Ok(())
}

pub fn spawn_test_server(port: u64) -> anyhow::Result<()> {
    let hostname = whoami::fallible::hostname().unwrap_or("unknown_host".to_string());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let test_server = TestRemoteServer {};

    let fut = async move {
        tracing::trace!("Spawning server!");
        println!("{hostname}:{port}");
        let listener = TcpListener::bind(format!("{hostname}:{port}"))
            .await
            .unwrap();
        Server::builder()
            .add_service(smelt_data::event_listener_server::EventListenerServer::new(
                test_server,
            ))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    };
    rt.block_on(fut);
    Ok(())
}
