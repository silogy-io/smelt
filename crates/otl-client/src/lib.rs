use async_trait::async_trait;
use std::{sync::Arc};

use otl_data::Event;
mod tracker;

#[async_trait]
pub(crate) trait Subscriber {
    async fn recv_event(event: Arc<Event>) -> ();

    async fn exit(&mut self) {
        
    }
}
