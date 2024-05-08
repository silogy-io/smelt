use std::sync::Arc;

use crate::Command;
use dice::{DiceData, DiceDataBuilder, UserComputationData};

use otl_data::Event;

use thiserror::Error;
use tokio::sync::mpsc::Sender;
mod local;
use async_trait::async_trait;
pub use local::LocalExecutorBuilder;

#[derive(Error, Debug)]
pub enum ExecutorErr {
    #[error("CommandIoError {0}")]
    CommandIOErr(#[from] std::io::Error),
}

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute_commands(
        &self,
        command: Arc<Command>,
        tx: Sender<Event>,
        dice_data: &UserComputationData,
    ) -> Result<Event, ExecutorErr>;
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
