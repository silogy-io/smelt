use libproc::{
    self,
    pid_rusage::{PIDRUsage, PidRUsageFlavor, RUsageInfoV2},
    processes,
};

struct SampleStruct {
    /// Memory used by a command in bytes
    ///
    /// currently calculated by summing the memory used by the command process and all of its
    /// children
    memory_used: u64,

    /// cpu load
    cpu_time_delta: u64,
}
fn sample_memory_and_load(pid: i32, previous_sample: Option<SampleStruct>) -> SampleStruct {
    let filter = libproc::processes::ProcFilter::ByParentProcess { ppid: pid as u32 };
    let mut timeused = 0;
    let mut memused = 0;

    if let Ok(pids) = processes::pids_by_type(filter) {
        for pid in pids {
            let val = libproc::pid_rusage::pidrusage::<RUsageInfoV2>(pid as i32).unwrap();
            memused += val.memory_used();
            timeused += val.ri_system_time + val.ri_user_time;
        }
    }
    let val = libproc::pid_rusage::pidrusage::<RUsageInfoV2>(pid).unwrap();

    timeused += val.ri_user_time + val.ri_system_time;
    if let Some(prev) = previous_sample {
        timeused = timeused - prev.cpu_time_delta;
    }
    SampleStruct {
        memory_used: memused,
        cpu_time_delta: timeused,
    }
}
