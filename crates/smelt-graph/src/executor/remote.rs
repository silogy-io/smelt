use std::{
    net::{SocketAddr, ToSocketAddrs},
    os::unix::fs::PermissionsExt,
};
use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use dice::{DiceData, UserComputationData};
use scc::HashMap;

use tokio::{
    sync::{mpsc::Sender, oneshot},
    task::JoinHandle,
};
use tonic::{transport::Server, Response};

use smelt_data::{
    executed_tests::{ExecutedTestResult, TestResult},
    Event, TaggedResult,
};
use smelt_events::runtime_support::{GetSmeltRoot, GetTraceId, GetTxChannel};

use crate::executor::Executor;
use crate::Command;

use super::common::{create_test_result, prepare_workspace, Workspace};

type TRMap = Arc<HashMap<String, tokio::sync::oneshot::Sender<TestResult>>>;

/// This is a dummy executor to test all of the logic of the slurm executor, with none of the
/// overhead of creating a slurm cluster
pub struct RemoteExecutor {
    binary_path: PathBuf,
}

#[derive(Debug, Clone)]
struct RemoteServer {
    tx_chan: Sender<Event>,
    connections: Arc<HashMap<String, tokio::sync::oneshot::Sender<TestResult>>>,
}

const WORKER_BIN: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_SMELT_SLURM_worker"));

async fn make_temp_executable(data: &[u8]) -> anyhow::Result<PathBuf> {
    let file = PathBuf::from(format!("{}/workerguy", std::env!("CARGO_MANIFEST_DIR")));

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
        let inner_event = request.into_inner();

        let _resp = self.tx_chan.send(inner_event).await;
        Ok(Response::new(()))
    }
    async fn send_outputs(
        &self,
        request: tonic::Request<TaggedResult>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let val = request.into_inner();
        let val = val.results.unwrap();
        let v = self.connections.remove(&val.test_name);
        match v {
            None => {
                tracing::error!("Missing entry in the remote server!");
                panic!();
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
                .serve(addr)
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
        let Workspace { .. } =
            prepare_workspace(command, root.clone(), command.working_dir.as_path()).await?;
        let (sender, rcv) = oneshot::channel();
        let _ = pertxstate.connections.insert(command.name.clone(), sender);
        let working_dir = command.default_target_root(root.as_path())?;

        let mut commandlocal = tokio::process::Command::new(self.binary_path.as_path());
        let arrrggs = [
            "--command-path".to_string(),
            working_dir.to_string_lossy().to_string(),
            "--command-name".to_string(),
            command.name.clone(),
            "--trace-id".to_string(),
            trace_id,
            "--host".to_string(),
            format!("http://{}", pertxstate.server_addr),
        ];

        commandlocal.args(arrrggs);
        let _handle = commandlocal.spawn().expect("Could not spawn!");

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
