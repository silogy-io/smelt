use std::time::Duration;

use bollard::container::StatsOptions;
use bollard::Docker;
use futures::TryStreamExt;
use libproc::{self, pid_rusage::PIDRUsage, processes};
use tokio::{sync::mpsc::Sender, time::Instant};

use smelt_data::{command_event::CommandVariant, CommandProfile, Event};

const MILIS_TO_NANOS: u64 = 1_000;
#[derive(Debug)]
struct SampleStruct {
    /// Memory used by a command in bytes
    ///
    /// currently calculated by summing the memory used by the command process and all of its
    /// children
    memory_used: u64,

    /// cpu time used, in nanoseconds
    cpu_time_delta: u64,
}

#[cfg(target_os = "linux")]
fn get_rusage_and_add(pid: i32, timeused: &mut u64, memused: &mut u64) {
    use libproc::pid_rusage::RUsageInfoV0;
    if let Some(val) = libproc::pid_rusage::pidrusage::<RUsageInfoV0>(pid as i32).ok() {
        *memused += val.memory_used();
        *timeused += (val.ri_system_time + val.ri_user_time);
    }
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

fn sample_memory_and_load(ppid: u32) -> Option<SampleStruct> {
    let filter = libproc::processes::ProcFilter::ByParentProcess { ppid };
    let mut timeused = 0;
    let mut memused = 0;

    if let Ok(pids) = processes::pids_by_type(filter) {
        for pid in pids {
            get_rusage_and_add(pid as i32, &mut timeused, &mut memused);
        }
    }

    get_rusage_and_add(ppid as i32, &mut timeused, &mut memused);

    Some(SampleStruct {
        memory_used: memused,
        cpu_time_delta: timeused,
    })
}

pub async fn profile_cmd_docker(
    tx: Sender<Event>,
    docker_client: Docker,
    sample_freq_ms: u64,
    command_ref: String,
    trace_id: String,
) -> i32 {
    println!("In profile_cmd_docker.");
    let stats = docker_client.stats(&command_ref, Some(StatsOptions {
        stream: false,
        ..Default::default()
    })).try_collect::<Vec<_>>().await.unwrap();
    for stat in stats {
        println!("{} - mem total: {:?} | mem usage: {:?}",
                 stat.name,
                 stat.memory_stats.max_usage,
                 stat.memory_stats.usage);
    }
    return 5;
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
        let new_sample = sample_memory_and_load(pid);
        let new_sample_time = Instant::now();
        if let Some(ref sample) = new_sample {
            if let Some(ref _prev) = prev_sample {
                let time_since = (new_sample_time - prev_sample_time).as_micros() as u64;
                let _ = tx
                    .send(profile_event(
                        &trace_id,
                        &command_ref,
                        &sample,
                        &_prev,
                        time_since,
                    ))
                    .await;
            }

            prev_sample = new_sample;
        }
        prev_sample_time = new_sample_time;

        tokio::time::sleep(Duration::from_millis(sample_freq_ms)).await;
    }
}

fn profile_event(
    trace_id: &String,
    command_ref: &String,
    sample: &SampleStruct,
    prev: &SampleStruct,
    sample_freq_ms: u64,
) -> Event {
    let variant = CommandVariant::Profile(CommandProfile {
        memory_used: sample.memory_used,
        cpu_load: ((sample.cpu_time_delta.saturating_sub(prev.cpu_time_delta)) as f32
            / MILIS_TO_NANOS as f32) as f32
            / sample_freq_ms as f32,
    });
    Event::from_command_variant(command_ref.clone(), trace_id.clone(), variant)
}
