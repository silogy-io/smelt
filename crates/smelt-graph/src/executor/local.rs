use std::process::Stdio;
use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use dice::{DiceData, UserComputationData};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::Sender,
};

use smelt_data::{
    executed_tests::{ExecutedTestResult, TestOutputs},
    Event,
};
use smelt_events::runtime_support::{
    GetProfilingFreq, GetSmeltCfg, GetSmeltRoot, GetTraceId, GetTxChannel, SlotController,
};

use crate::executor::{common::handle_line, Executor};
use crate::Command;

use super::common::{create_test_result, prepare_workspace, Workspace};
use smelt_rt::profile_cmd;

pub struct LocalExecutor {}

#[async_trait]
impl Executor for LocalExecutor {
    async fn execute_commands(
        &self,
        command: Arc<Command>,
        dd: &UserComputationData,
        global_data: &DiceData,
    ) -> anyhow::Result<ExecutedTestResult> {
        let tx = dd.get_tx_channel();
        let local_command = command;
        let trace_id = dd.get_trace_id();
        let root = global_data.get_smelt_root();

        let rv = execute_local_command(
            local_command.as_ref(),
            trace_id.clone(),
            tx.clone(),
            root,
            global_data,
        )
        .await
        .map(|output| create_test_result(local_command.as_ref(), output.exit_code, global_data))?;
        Ok(rv)
    }
}

async fn execute_local_command(
    command: &Command,
    trace_id: String,
    tx_chan: Sender<Event>,
    root: PathBuf,
    global_data: &DiceData,
) -> anyhow::Result<TestOutputs> {
    let silent = global_data.get_smelt_cfg().silent;
    let _sem = global_data.acquire_slots(command.runtime.num_cpus).await;
    let shell = "bash";
    let _handle_me = tx_chan
        .send(Event::command_started(
            command.name.clone(),
            trace_id.clone(),
        ))
        .await;

    let Workspace {
        script_file,
        mut stdout,
        ..
    } = prepare_workspace(command, root.clone(), command.working_dir.as_path()).await?;

    let mut commandlocal = tokio::process::Command::new(shell);

    commandlocal
        .arg(script_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if global_data.get_smelt_cfg().sandbox_env {
        commandlocal.env_clear();
    }
    let mut comm_handle = commandlocal.spawn()?;
    let stderr = comm_handle.stderr.take().unwrap();
    let stderr_reader = BufReader::new(stderr);
    let mut stderr_lines = stderr_reader.lines();

    let reader = BufReader::new(comm_handle.stdout.take().unwrap());
    let mut lines = reader.lines();
    let maybe_pid = comm_handle.id();

    let sample_task = maybe_pid.and_then(|pid| {
        global_data.get_profiling_freq().map(|freq| {
            tokio::spawn(profile_cmd(
                pid,
                tx_chan.clone(),
                freq,
                command.name.clone(),
                trace_id.clone(),
            ))
        })
    });

    let cstatus: TestOutputs = loop {
        tokio::select!(
            Ok(Some(line)) = lines.next_line() => {
                handle_line(command,line,trace_id.clone(),&tx_chan,&mut stdout, silent).await;
            }
            Ok(Some(line)) = stderr_lines.next_line() => {
                handle_line(command,line,trace_id.clone(),&tx_chan,&mut stdout, silent).await;
            }
            status_code = comm_handle.wait() => {
                break status_code.map(|val| TestOutputs{ exit_code: val.code().unwrap_or(-555), artifacts: vec![]});
            }


        );
    }?;
    //kill the sampling task
    if let Some(task) = sample_task {
        task.abort()
    }

    while let Ok(Some(line)) = lines.next_line().await {
        handle_line(
            command,
            line,
            trace_id.clone(),
            &tx_chan,
            &mut stdout,
            silent,
        )
        .await;
    }

    while let Ok(Some(line)) = stderr_lines.next_line().await {
        handle_line(
            command,
            line,
            trace_id.clone(),
            &tx_chan,
            &mut stdout,
            silent,
        )
        .await;
    }

    Ok(cstatus)
}
