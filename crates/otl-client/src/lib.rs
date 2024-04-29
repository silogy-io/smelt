use async_trait::async_trait;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use otl_data::Event;
mod console;
mod tracker;

/// Information about tick timing.
#[derive(Debug, Clone)]
pub struct Tick {
    /// The time that the ticker was started.
    pub start_time: Instant,
    /// Elapsed time since the ticker was started for this tick.
    pub elapsed_time: Duration,
}

#[async_trait]
pub trait Subscriber: Send {
    async fn recv_event(&mut self, event: Arc<Event>) -> Result<(), anyhow::Error>;

    async fn exit(&mut self) {}
}
