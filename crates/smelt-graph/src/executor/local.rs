use crate::executor::{common::handle_line, Executor};
use dice::{DiceData, UserComputationData};
use std::process::Stdio;
use std::{path::PathBuf, sync::Arc};

use crate::Command;
use async_trait::async_trait;

use smelt_data::{
    executed_tests::{ExecutedTestResult},
    CommandOutput, Event,
};
use smelt_events::{
    runtime_support::{GetCmdDefPath, GetSmeltRoot, GetTraceId, GetTxChannel},
    to_file,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::Sender,
};

use super::common::{create_test_result, prepare_workspace, Workspace};

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
        let command_default_dir = global_data.get_cmd_def_path();
        let rv = execute_local_command(
            local_command.as_ref(),
            trace_id.clone(),
            tx.clone(),
            command_default_dir,
            root,
        )
        .await
        .map(|output| {
            create_test_result(local_command.as_ref(), output.status_code, global_data)
        })?;
        Ok(rv)
    }
}

async fn execute_local_command(
    command: &Command,
    trace_id: String,
    tx_chan: Sender<Event>,
    command_working_dir: PathBuf,
    root: PathBuf,
) -> anyhow::Result<CommandOutput> {
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
        working_dir,
    } = prepare_workspace(command, root.clone()).await?;

    let mut commandlocal = tokio::process::Command::new(shell);

    commandlocal
        .arg(script_file)
        .current_dir(command_working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut comm_handle = commandlocal.spawn()?;
    let stderr = comm_handle.stderr.take().unwrap();
    let stderr_reader = BufReader::new(stderr);
    let mut stderr_lines = stderr_reader.lines();

    let reader = BufReader::new(comm_handle.stdout.take().unwrap());
    let mut lines = reader.lines();

    let cstatus: CommandOutput = loop {
        tokio::select!(
            Ok(Some(line)) = lines.next_line() => {
                handle_line(command,line,trace_id.clone(),&tx_chan,&mut stdout).await;
            }
            Ok(Some(line)) = stderr_lines.next_line() => {
                handle_line(command,line,trace_id.clone(),&tx_chan,&mut stdout).await;
            }
            status_code = comm_handle.wait() => {
                break status_code.map(|val| CommandOutput { status_code: val.code().unwrap_or(-555)});
            }


        );
    }?;

    while let Ok(Some(line)) = lines.next_line().await {
        handle_line(command, line, trace_id.clone(), &tx_chan, &mut stdout).await;
    }

    to_file(&cstatus, &working_dir).await?;
    Ok(cstatus)
}
