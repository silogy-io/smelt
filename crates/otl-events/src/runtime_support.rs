use crate::Event;
use dice::UserComputationData;

use tokio::sync::mpsc::Sender;

pub trait SetTxChannel {
    fn set_tx_channel(&mut self, tx_channel: Sender<Event>);
}
pub trait GetTxChannel {
    fn get_tx_channel(&self) -> Sender<Event>;
}

impl SetTxChannel for UserComputationData {
    fn set_tx_channel(&mut self, tx_channel: Sender<Event>) {
        self.data.set(tx_channel);
    }
}

impl GetTxChannel for UserComputationData {
    fn get_tx_channel(&self) -> Sender<Event> {
        self.data
            .get::<Sender<Event>>()
            .expect("Channel should be set")
            .clone()
    }
}
