use std::sync::Arc;

use crate::Command;
use dice::{DiceData, DiceDataBuilder};
use futures::Stream;
use otl_data::Event;

use thiserror::Error;
use tokio::sync::mpsc::Receiver;
mod local;
pub use local::LocalExecutorBuilder;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Error, Debug)]
pub enum ExecutorErr {
    #[error("CommandIoError {0}")]
    CommandIOErr(#[from] std::io::Error),
}

pub trait Executor {
    async fn execute_commands(&self, command: Arc<Command>)
        -> Result<Receiver<Event>, ExecutorErr>;
    async fn command_as_stream(
        &self,
        command: Arc<Command>,
    ) -> Result<impl Stream<Item = Event>, ExecutorErr> {
        let chan = self.execute_commands(command).await?;
        Ok(ReceiverStream::new(chan))
    }
}

// We use this instead of Box<dyn Executor> because trait objects with async methods aren't
// supported yet
//
// see https://rust-lang.github.io/async-fundamentals-initiative/explainer/async_fn_in_dyn_trait.html
//
// This should cover all types of executors we implement
pub enum ExecutorShim {
    Local(local::LocalExecutor),
}

impl Executor for ExecutorShim {
    async fn execute_commands(
        &self,
        command: Arc<Command>,
    ) -> Result<Receiver<Event>, ExecutorErr> {
        match self {
            Self::Local(local_exec) => local_exec.execute_commands(command).await,
        }
    }
}

pub trait SetExecutor {
    fn set_executor(&mut self, exec: impl Into<ExecutorShim>);
}

pub trait GetExecutor {
    fn get_executor(&self) -> Arc<ExecutorShim>;
}

impl SetExecutor for DiceDataBuilder {
    fn set_executor(&mut self, exec: impl Into<ExecutorShim>) {
        self.set(Arc::new(exec.into()))
    }
}

impl GetExecutor for DiceData {
    fn get_executor(&self) -> Arc<ExecutorShim> {
        self.get::<Arc<ExecutorShim>>()
            .expect("Channel should be set")
            .clone()
    }
}
