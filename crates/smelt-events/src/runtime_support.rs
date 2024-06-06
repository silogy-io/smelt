use std::path::{Path, PathBuf};

use crate::Event;
use dice::{DiceData, DiceDataBuilder, UserComputationData};
use smelt_core::SmeltPath;
use smelt_data::client_commands::ConfigureSmelt;
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

pub trait SetSmeltCfg {
    fn set_smelt_cfg(&mut self, cfg: ConfigureSmelt);
}

pub trait GetSmeltCfg {
    fn get_smelt_cfg(&self) -> &ConfigureSmelt;
}

pub trait GetSmeltRoot {
    fn get_smelt_root(&self) -> PathBuf;
}

pub trait GetProfilingFreq {
    /// Gets the profiling frequency in milliseconds
    ///
    /// If not set, then profiling is disabled
    fn get_profiling_freq(&self) -> Option<u64>;
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

impl GetSmeltRoot for DiceData {
    fn get_smelt_root(&self) -> PathBuf {
        self.get::<ConfigureSmelt>()
            .map(|val| PathBuf::from(&val.smelt_root))
            .unwrap()
    }
}

impl SetSmeltCfg for DiceDataBuilder {
    fn set_smelt_cfg(&mut self, cfg: ConfigureSmelt) {
        self.set(cfg)
    }
}

impl GetCmdDefPath for DiceData {
    fn get_cmd_def_path(&self) -> PathBuf {
        self.get::<ConfigureSmelt>()
            .map(|val| {
                SmeltPath::new(val.smelt_root.clone()).to_path(Path::new(val.smelt_root.as_str()))
            })
            .unwrap()
    }
}

impl GetSmeltCfg for DiceData {
    fn get_smelt_cfg(&self) -> &ConfigureSmelt {
        self.get::<ConfigureSmelt>()
            .expect("Cfg object should be set")
    }
}
use smelt_data::client_commands::ProfilingSelection;
impl GetProfilingFreq for DiceData {
    fn get_profiling_freq(&self) -> Option<u64> {
        self.get_smelt_cfg().prof_cfg.as_ref().and_then(|val| {
            if val.prof_type == ProfilingSelection::Disabled as i32 {
                None
            } else {
                Some(val.sampling_period)
            }
        })
    }
}
