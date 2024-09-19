use std::{fs::set_permissions, net::SocketAddr, os::unix::fs::PermissionsExt, path::Path};
use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use dice::{DiceData, UserComputationData};
use scc::HashMap;
use smelt_core::{get_target_root, SmeltErr};

use std::io::Write;

use tokio::{fs::File, io::AsyncWriteExt, net::TcpListener};

use tokio::{
    sync::{mpsc::Sender, oneshot},
    task::JoinHandle,
};
use tonic::{transport::Server, Response};

use smelt_data::{
    client_commands::{
        cfg_slurm::SealedWorkspace, configure_smelt::InitExecutor, CfgSlurm, ConfigureSmelt,
        DockerWorkspace,
    },
    executed_tests::{ExecutedTestResult, TestResult},
    Event,
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
    server_addr: SocketAddr,
    server_handle: JoinHandle<()>,
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
        request: tonic::Request<TestResult>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let val = request.into_inner();
        println!("result is {:?}", val);
        Ok(Response::new(()))
    }
}

#[tonic::async_trait]
impl smelt_data::event_listener_server::EventListener for RemoteServer {
    async fn send_event(
        &self,
        request: tonic::Request<Event>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let inner_event = request.into_inner();
        let _resp = self.tx_chan.send(inner_event).await;
        Ok(Response::new(()))
    }
    async fn send_outputs(
        &self,
        request: tonic::Request<TestResult>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let val = request.into_inner();
        tracing::trace!("Trying to remove {}", val.test_name);
        let v = self.connections.remove_async(&val.test_name).await;
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
        data.get_pertx_state().server_handle.abort();
    }

    async fn init_per_tx_state(&self, data: &mut UserComputationData) {
        let tx_chan = data.get_tx_channel();
        let connections = Arc::new(HashMap::new());
        let remote_server = RemoteServer {
            tx_chan,
            connections: connections.clone(),
        };

        let hn = data.get_hostname();
        let listener = TcpListener::bind(format!("{hn}:0")).await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = tokio::spawn(async move {
            tracing::trace!("Spawning server!");
            Server::builder()
                .add_service(smelt_data::event_listener_server::EventListenerServer::new(
                    remote_server,
                ))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        let pertx = PerTxRemoteState {
            connections,
            server_addr: addr,
            server_handle,
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
        let addr = pertxstate.server_addr;

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

                commandlocal.spawn()?
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
                commandlocal.arg("--output=/dev/null");
                commandlocal.arg("--error=/dev/null");
                commandlocal.arg(format!("--wrap={}", command));

                commandlocal.spawn()?
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
