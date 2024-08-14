use std::{
    net::{SocketAddr, ToSocketAddrs},
    os::unix::fs::PermissionsExt,
    process::Stdio,
};
use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use dice::{DiceData, UserComputationData};
use scc::HashMap;
use tempfile::{tempfile, NamedTempFile};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{mpsc::Sender, oneshot},
    task::JoinHandle,
};
use tonic::{transport::Server, Request, Response, Status, Streaming};

use smelt_data::{
    executed_tests::{ExecutedTestResult, TestOutputs, TestResult},
    Event,
};
use smelt_events::runtime_support::{
    GetProfilingFreq, GetSmeltCfg, GetSmeltRoot, GetTraceId, GetTxChannel, SlotController,
};

use crate::executor::{common::handle_line, Executor};
use crate::Command;

use super::{
    common::{create_test_result, prepare_workspace, Workspace},
    profiler::profile_cmd,
};

type TRMap = Arc<HashMap<String, tokio::sync::oneshot::Sender<TestResult>>>;

/// This is a dummy executor to test all of the logic of the slurm executor, with none of the
/// overhead of creating a slurm cluster
pub struct RemoteExecutor {
    binary_path: NamedTempFile,
}

#[derive(Debug, Clone)]
struct RemoteServer {
    tx_chan: Sender<Event>,
    connections: Arc<HashMap<String, tokio::sync::oneshot::Sender<TestResult>>>,
}

const WORKER_BIN: &'static [u8] = include_bytes!(env!("CARGO_BIN_FILE_SMELT_SLURM_worker"));

async fn make_temp_executable(data: &[u8]) -> anyhow::Result<NamedTempFile> {
    let file = tempfile::NamedTempFile::new()?;
    tokio::fs::write(file.path(), data).await?;
    let mut perms = tokio::fs::metadata(file.path()).await?.permissions();
    perms.set_mode(0o755); // make exec
    let _ = tokio::fs::set_permissions(file.path(), perms).await?;
    Ok(file)
}

struct PerTxRemoteState {
    connections: TRMap,
    server_addr: SocketAddr,
    server_handle: JoinHandle<()>,
}

impl RemoteExecutor {
    pub async fn new() -> Self {
        let res = make_temp_executable(WORKER_BIN).await.unwrap();
        Self { binary_path: res }
    }
}

#[tonic::async_trait]
impl smelt_data::event_listener_server::EventListener for RemoteServer {
    async fn send_event(
        &self,
        request: tonic::Request<Event>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let val = request.into_inner();
        let _val = self.tx_chan.send(val).await;
        Ok(Response::new(()))
    }
    async fn send_outputs(
        &self,
        request: tonic::Request<TestResult>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let val = request.into_inner();
        let v = self.connections.remove(&val.test_name);
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
impl Executor for RemoteExecutor {
    async fn init_per_tx_state(&self, data: &mut UserComputationData) {
        // This is bad! we could collide on port! I dont care
        let port = 9213;
        let tx_chan = data.get_tx_channel();
        let connections = Arc::new(HashMap::new());
        let remote_server = RemoteServer {
            tx_chan,
            connections: connections.clone(),
        };

        let addr = format!("0.0.0.0:{port}")
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap();
        let server_handle = tokio::spawn(async move {
            Server::builder()
                .add_service(smelt_data::event_listener_server::EventListenerServer::new(
                    remote_server,
                ))
                .serve(addr.clone())
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
        let tx = dd.get_tx_channel();

        let trace_id = dd.get_trace_id();
        let root = global_data.get_smelt_root();
        let command = command.as_ref();
        let pertxstate = dd.get_pertx_state();
        let Workspace { script_file, .. } =
            prepare_workspace(command, root.clone(), command.working_dir.as_path()).await?;
        let (sender, rcv) = oneshot::channel();
        let _ = pertxstate.connections.insert(command.name.clone(), sender);

        let mut commandlocal = tokio::process::Command::new(self.binary_path.path());
        commandlocal.args([
            format!("--comand_path {}", script_file.to_string_lossy()),
            format!("--comand_name {}", command.name),
            format!("--trace_id {}", trace_id),
            format!("--host {}", pertxstate.server_addr.to_string()),
        ]);
        let handle = commandlocal.spawn().expect("Could not spawn!");

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
