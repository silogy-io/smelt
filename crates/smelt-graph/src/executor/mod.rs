use std::sync::Arc;

use crate::Command;
use dice::{DiceData, DiceDataBuilder, UserComputationData};

use smelt_data::executed_tests::ExecutedTestResult;

mod common;

#[cfg(feature = "docker")]
mod docker;
mod local;
#[cfg(test)]
mod remote;

mod slurm;

use async_trait::async_trait;
#[cfg(feature = "docker")]
pub use docker::DockerExecutor;
pub use local::LocalExecutor;
#[cfg(test)]
pub use remote::RemoteExecutor;
pub use slurm::{init_worker_binary, spawn_test_server, SlurmExecutor, WORKER_BIN};

#[async_trait]
pub trait Executor: Send + Sync {
    /// The heavy lifting of actually _executing_ a command is implemented here
    /// the [`dice_data`](UserComputationData) contains per invocation metadata
    /// (e.g. unique id of the invocation)
    async fn execute_commands(
        &self,
        command: Arc<Command>,
        dice_data: &UserComputationData,
        global_dice_data: &DiceData,
    ) -> anyhow::Result<ExecutedTestResult>;

    /// Initialization of per execution state. This is particularly useful for executors that need
    /// to create transient services see the slurm executor
    async fn init_per_tx_state(&self, _dice_data: &mut UserComputationData) {}

    /// The "free"-ing side of the per tx initialization
    async fn drop_per_tx_state(&self, _dice_data: &UserComputationData) {}
}

pub trait SetExecutor {
    fn set_executor(&mut self, exec: Arc<dyn Executor>);
}

pub trait GetExecutor {
    fn get_executor(&self) -> Arc<dyn Executor>;
}

impl SetExecutor for DiceDataBuilder {
    fn set_executor(&mut self, exec: Arc<dyn Executor>) {
        self.set(exec)
    }
}

impl GetExecutor for DiceData {
    fn get_executor(&self) -> Arc<dyn Executor> {
        self.get::<Arc<dyn Executor>>()
            .expect("Channel should be set")
            .clone()
    }
}
