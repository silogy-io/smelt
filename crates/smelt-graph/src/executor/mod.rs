use std::sync::Arc;

use crate::Command;
use dice::{DiceData, DiceDataBuilder, UserComputationData};

use smelt_data::executed_tests::ExecutedTestResult;

mod common;

#[cfg(feature = "docker")]
mod docker;
mod local;
mod profiler;

use async_trait::async_trait;
#[cfg(feature = "docker")]
pub use docker::DockerExecutor;
pub use local::LocalExecutor;

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute_commands(
        &self,
        command: Arc<Command>,
        dice_data: &UserComputationData,
        global_dice_data: &DiceData,
    ) -> anyhow::Result<ExecutedTestResult>;
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
