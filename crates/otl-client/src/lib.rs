use async_trait::async_trait;
use std::sync::Arc;

use otl_data::Event;
mod console;
mod tracker;

#[async_trait]
pub trait Subscriber: Send {
    async fn recv_event(&mut self, event: Arc<Event>) -> Result<(), anyhow::Error>;

    async fn exit(&mut self) {}
}
