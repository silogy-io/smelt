use std::path::{Path, PathBuf};

use crate::Event;
use dice::{DiceData, DiceDataBuilder, UserComputationData};
use otl_core::OtlPath;
use otl_data::client_commands::ConfigureOtl;
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

pub trait SetOtlCfg {
    fn set_otl_cfg(&mut self, cfg: ConfigureOtl);
}

pub trait GetOtlCfg {
    fn get_otl_cfg(&mut self) -> &ConfigureOtl;
}

pub trait GetOtlRoot {
    fn get_otl_root(&self) -> PathBuf;
}

pub trait GetCmdDefPath {
    fn get_cmd_def_path(&self) -> PathBuf;
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

impl GetOtlRoot for DiceData {
    fn get_otl_root(&self) -> PathBuf {
        self.get::<ConfigureOtl>()
            .map(|val| PathBuf::from(&val.otl_root))
            .unwrap()
    }
}

impl SetOtlCfg for DiceDataBuilder {
    fn set_otl_cfg(&mut self, cfg: ConfigureOtl) {
        self.set(cfg)
    }
}

impl GetCmdDefPath for DiceData {
    fn get_cmd_def_path(&self) -> PathBuf {
        self.get::<ConfigureOtl>()
            .map(|val| OtlPath::new(val.otl_root.clone()).to_path(Path::new(val.otl_root.as_str())))
            .unwrap()
    }
}

impl GetOtlCfg for DiceData {
    fn get_otl_cfg(&mut self) -> &ConfigureOtl {
        self.get::<ConfigureOtl>().expect("Trace id should be set")
    }
}
