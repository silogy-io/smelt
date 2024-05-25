use std::path::PathBuf;

use crate::Event;
use dice::{DiceData, DiceDataBuilder, UserComputationData};
use uuid::Uuid;

use tokio::sync::mpsc::Sender;

pub trait SetTxChannel {
    fn set_tx_channel(&mut self, tx_channel: Sender<Event>);
}
pub trait GetTxChannel {
    fn get_tx_channel(&self) -> Sender<Event>;
}

pub trait SetTraceId {
    fn init_trace_id(&mut self);
}

pub trait GetTraceId {
    fn get_trace_id(&self) -> String;
}

pub trait SetOtlRoot {
    fn set_otl_root(&mut self, pathbuf: PathBuf);
}

pub trait GetOtlRoot {
    fn get_otl_root(&self) -> PathBuf;
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

struct LocalUuid(String);
impl SetTraceId for UserComputationData {
    fn init_trace_id(&mut self) {
        let luid = LocalUuid(Uuid::new_v4().to_string());
        self.data.set(luid);
    }
}

impl GetTraceId for UserComputationData {
    fn get_trace_id(&self) -> String {
        self.data
            .get::<LocalUuid>()
            .expect("Trace id should be set")
            .0
            .clone()
    }
}

struct OtlRootHolder(PathBuf);

impl SetOtlRoot for DiceDataBuilder {
    fn set_otl_root(&mut self, pathbuf: PathBuf) {
        self.set(OtlRootHolder(pathbuf))
    }
}

impl GetOtlRoot for DiceData {
    fn get_otl_root(&self) -> PathBuf {
        self.get::<OtlRootHolder>()
            .expect("Trace id should be set")
            .0
            .clone()
    }
}
