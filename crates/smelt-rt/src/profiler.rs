use std::time::Duration;

use bollard::container::{MemoryStatsStats, Stats, StatsOptions};
use bollard::Docker;
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use libproc::{self, pid_rusage::PIDRUsage, processes};
use tokio::{sync::mpsc::Sender, time::Instant};

use smelt_data::{command_event::CommandVariant, CommandProfile, Event};

const NANOS_TO_MICROS: u64 = 1_000;
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

async fn docker_sample(docker_client: &Docker, command_ref: &String) -> Option<Stats> {
    // TODO This should return a stream of stats. AFAIK the way Docker stats
    //  works is that the daemon constantly polls each container once per
    //  second for statistics. The /stats/ API call, once it starts, waits for
    //  the daemon to poll twice in order to gather data for two consecutive
    //  ticks, and returns data from both; this is to allow the client to
    //  compute rate-related information such as CPU load. This means that if
    //  we set stream=False, our rate of fetching stats is limited to once
    //  every _two_ seconds.
    let stats = docker_client
        .stats(
            command_ref,
            Some(StatsOptions {
                stream: false,
                ..Default::default()
            }),
        )
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    for stat in stats {
        return Some(stat);
    }
    None
}

fn docker_stats_to_event(
    trace_id: &String,
    command_ref: &String,
    stats: &Stats,
    profile_start_time_ms: u64,
) -> Option<Event> {
    let parsed = stats.read.parse::<DateTime<Utc>>();
    // Sometimes the first Stats object returned by the library has everything zeroed out, with a
    // date of 0001-01-01T00:00:00Z, which corresponds to a negative timestamp. We ignore these.
    let sample_timestamp_ms: u64 = parsed
        .expect("failed to parse datetime")
        .timestamp_millis()
        .try_into()
        .ok()?;

    docker_profile_event(
        trace_id,
        command_ref,
        stats,
        sample_timestamp_ms.saturating_sub(profile_start_time_ms),
    )
}

pub async fn profile_cmd_docker(
    tx: Sender<Event>,
    docker_client: Docker,
    container_name: &String,
    command_ref: String,
    trace_id: String,
    profile_start_time_ms: u64,
) {
    loop {
        // TODO This should instead use the streaming version of the stats endpoint
        let new_sample = docker_sample(&docker_client, container_name).await;

        if let Some(ref stats) = new_sample {
            if let Some(event) =
                docker_stats_to_event(&trace_id, &command_ref, stats, profile_start_time_ms)
            {
                let _ = tx.send(event).await;
            }
        }
    }
}

fn docker_profile_event(
    trace_id: &String,
    command_ref: &String,
    stats: &Stats,
    time_since_start_ms: u64,
) -> Option<Event> {
    // Calculations based on https://docs.docker.com/engine/api/v1.45/#tag/Container/operation/ContainerStats
    let cpu_delta_us =
        stats.cpu_stats.cpu_usage.total_usage - stats.precpu_stats.cpu_usage.total_usage;
    let system_cpu_delta_us = match (
        stats.cpu_stats.system_cpu_usage,
        stats.precpu_stats.system_cpu_usage,
    ) {
        (None, _) => return None,
        (_, None) => return None,
        (Some(system_cpu_usage), Some(prev_system_cpu_usage)) => {
            system_cpu_usage.saturating_sub(prev_system_cpu_usage)
        }
    };

    let number_cpus = match (
        stats.cpu_stats.online_cpus,
        stats.cpu_stats.cpu_usage.percpu_usage.clone(),
    ) {
        (Some(cpus), _) => cpus,
        (_, Some(percpu_usage)) => percpu_usage.len().try_into().unwrap(),
        (_, _) => return None,
    };
    let cpu_load = cpu_delta_us as f32 / system_cpu_delta_us as f32 * number_cpus as f32;

    let memory_stats = stats.memory_stats;
    let memory_used = match (memory_stats.usage, memory_stats.stats) {
        (None, _) => return None,
        (Some(usage), None) => usage,
        // From https://docs.docker.com/reference/cli/docker/container/stats/:
        // See On Docker 19.03 and older, the cache usage was defined as the
        // value of cache field. On cgroup v2 hosts, the cache usage is defined
        // as the value of inactive_file field.
        (Some(usage), Some(MemoryStatsStats::V1(mem_stats_v1))) => usage - mem_stats_v1.cache,
        (Some(usage), Some(MemoryStatsStats::V2(mem_stats_v2))) => {
            usage - mem_stats_v2.inactive_file
        }
    };

    let variant = CommandVariant::Profile(CommandProfile {
        memory_used,
        cpu_load,
        time_since_start_ms,
    });
    Some(Event::from_command_variant(
        command_ref.clone(),
        trace_id.clone(),
        variant,
    ))
}

pub async fn profile_cmd(
    pid: u32,
    tx: Sender<Event>,
    sample_freq_ms: u64,
    command_ref: String,
    trace_id: String,
) {
    let start_sample_time = Instant::now();
    let mut prev_sample = None;
    let mut prev_sample_time = Instant::now();

    loop {
        let new_sample = sample_memory_and_load(pid);
        let new_sample_time = Instant::now();
        if let Some(ref sample) = new_sample {
            if let Some(ref _prev) = prev_sample {
                let time_since_previous = (new_sample_time - prev_sample_time).as_micros() as u64;
                let time_since_start = (new_sample_time - start_sample_time).as_millis() as u64;
                let _ = tx
                    .send(profile_event(
                        &trace_id,
                        &command_ref,
                        sample,
                        _prev,
                        time_since_previous,
                        time_since_start,
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
    time_since_previous_us: u64,
    time_since_start_ms: u64,
) -> Event {
    let variant = CommandVariant::Profile(CommandProfile {
        memory_used: sample.memory_used,
        // Microseconds of CPU time / microseconds of wall time
        cpu_load: ((sample.cpu_time_delta.saturating_sub(prev.cpu_time_delta)) as f32
            / NANOS_TO_MICROS as f32)
            / time_since_previous_us as f32,
        time_since_start_ms,
    });
    Event::from_command_variant(command_ref.clone(), trace_id.clone(), variant)
}
