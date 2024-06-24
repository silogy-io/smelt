use std::path::{Path, PathBuf};

use crate::Event;
use async_trait::async_trait;
use dice::{DiceData, DiceDataBuilder, UserComputationData};
use smelt_core::SmeltPath;
use smelt_data::client_commands::ConfigureSmelt;
use tokio::sync::{Semaphore, SemaphorePermit};
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

pub trait SetSemaphore {
    fn set_sempahore(&mut self, cnt: usize);
}

#[async_trait]
pub trait LockSemaphore {
    /// Gets the semaphore we use to control how many slots we're using in smelt
    async fn lock_sem(&self, cnt: u32) -> SemaphorePermit<'_>;
}
pub trait GetJobSlots {
    fn get_job_slots(&self) -> u64;
}

pub trait GetProfilingFreq {
    /// Gets the profiling frequency in milliseconds
    ///
    /// If not set, then profiling is disabled
    fn get_profiling_freq(&self) -> Option<u64>;
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

impl GetJobSlots for DiceData {
    fn get_job_slots(&self) -> u64 {
        self.get_smelt_cfg().job_slots
    }
}

impl SetSmeltCfg for DiceDataBuilder {
    fn set_smelt_cfg(&mut self, cfg: ConfigureSmelt) {
        let max = cfg.job_slots as usize;
        self.set(cfg);
        self.set_sempahore(max);
    }
}

impl SetSemaphore for DiceDataBuilder {
    fn set_sempahore(&mut self, cnt: usize) {
        let sem = Semaphore::new(cnt);
        self.set(sem);
    }
}
#[async_trait]
impl LockSemaphore for DiceData {
    async fn lock_sem(&self, cnt: u32) -> SemaphorePermit<'_> {
        let sem = self.get::<Semaphore>().expect("Semaphore should be set");
        let max_slots = self.get_smelt_cfg().job_slots;
        let slots = cnt.min(max_slots as u32);

        let available = sem.available_permits();
        tracing::debug!("Acquiring semaphore {cnt}, max is {max_slots}, current is {available}");
        let val = sem
            .acquire_many(slots)
            .await
            .expect("We should NEVER close this semaphore");

        val
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
