use std::time::Duration;

use libproc::{self, pid_rusage::PIDRUsage, processes};
use smelt_data::{command_event::CommandVariant, CommandProfile, Event};
use tokio::{sync::mpsc::Sender, time::Instant};
const MICROS_TO_NANOS: u64 = 1_000;
struct SampleStruct {
    /// Memory used by a command in bytes
    ///
    /// currently calculated by summing the memory used by the command process and all of its
    /// children
    memory_used: u64,

    /// cpu load
    cpu_time_delta: u64,
}

#[cfg(target_os = "macos")]
fn get_rusage_and_add(pid: i32, timeused: &mut u64, memused: &mut u64) {
    use libproc::pid_rusage::RUsageInfoV3;
    use mach2::mach_time::mach_timebase_info;

    let mut timebase = mach_timebase_info::default();

    unsafe { mach_timebase_info(&mut timebase) };
    if let Ok(val) = libproc::pid_rusage::pidrusage::<RUsageInfoV3>(pid) {
        *memused += val.memory_used();
        *timeused += ((val.ri_system_time + val.ri_user_time) * timebase.numer as u64)
            / timebase.denom as u64;
    }
}

#[cfg(target_os = "linux")]
fn get_rusage_and_add(pid: i32, timeused: &mut u64, memused: &mut u64) {
    use libproc::pid_rusage::RUsageInfoV0;
    if let Some(val) = libproc::pid_rusage::pidrusage::<RUsageInfoV0>(pid as i32).ok() {
        *memused += val.memory_used();
        *timeused += (val.ri_system_time + val.ri_user_time) / MICROS_TO_NANOS;
    }
}

fn sample_memory_and_load(
    pid: u32,
    previous_sample: &Option<SampleStruct>,
) -> Option<SampleStruct> {
    let filter = libproc::processes::ProcFilter::ByParentProcess { ppid: pid };
    let mut timeused = 0;
    let mut memused = 0;

    if let Ok(pids) = processes::pids_by_type(filter) {
        for pid in pids {
            get_rusage_and_add(pid as i32, &mut timeused, &mut memused);
        }
    }

    get_rusage_and_add(pid as i32, &mut timeused, &mut memused);

    if let Some(prev) = previous_sample {
        timeused -= prev.cpu_time_delta;
    }
    Some(SampleStruct {
        memory_used: memused,
        cpu_time_delta: timeused,
    })
}

pub async fn profile_cmd(
    pid: u32,
    tx: Sender<Event>,
    sample_freq_ms: u64,
    command_ref: String,
    trace_id: String,
) {
    let mut prev_sample = None;
    let mut prev_sample_time = Instant::now();

    loop {
        let new_sample = sample_memory_and_load(pid, &prev_sample);
        let new_sample_time = Instant::now();
        if let Some(sample) = new_sample {
            if let Some(ref _prev) = prev_sample {
                let time_since = (new_sample_time - prev_sample_time).as_micros() as u64;
                let _ = tx
                    .send(profile_event(&trace_id, &command_ref, &sample, time_since))
                    .await;
            }
            prev_sample = Some(sample);
        }
        prev_sample_time = new_sample_time;

        tokio::time::sleep(Duration::from_millis(sample_freq_ms)).await;
    }
}

fn profile_event(
    trace_id: &String,
    command_ref: &String,
    sample: &SampleStruct,
    sample_freq_ms: u64,
) -> Event {
    let variant = CommandVariant::Profile(CommandProfile {
        memory_used: sample.memory_used,
        cpu_load: (sample.cpu_time_delta / MICROS_TO_NANOS) as f32 / sample_freq_ms as f32,
    });
    Event::from_command_variant(command_ref.clone(), trace_id.clone(), variant)
}
