use async_trait::async_trait;
use std::sync::Arc;

use otl_data::Event;
mod console;
mod tracker;

#[async_trait]
pub(crate) trait Subscriber {
    type Error;
    async fn recv_event(&mut self, event: Arc<Event>) -> Result<(), Self::Error>;

    async fn tick(&mut self) {}

    async fn exit(&mut self) {}
}
