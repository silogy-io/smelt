use crate::executor::{common::handle_line, Executor};
use std::{io::Write, process::Stdio};
use std::{path::PathBuf, sync::Arc};

use crate::Command;
use async_trait::async_trait;
use dice::UserComputationData;
use otl_core::OtlErr;
use otl_data::{CommandOutput, Event};
use otl_events::{runtime_support::GetTraceId, to_file};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc::Sender,
};

use super::common::{prepare_workspace, Workspace};

pub struct LocalExecutorBuilder {
    threads: usize,
}

impl LocalExecutorBuilder {
    pub fn new() -> Self {
        Self { threads: 4 }
    }
    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    pub fn build(self) -> Result<LocalExecutor, OtlErr> {
        Ok(LocalExecutor {})
    }
}

pub struct LocalExecutor {}

#[async_trait]
impl Executor for LocalExecutor {
    async fn execute_commands(
        &self,
        command: Arc<Command>,
        tx: Sender<Event>,
        dd: &UserComputationData,
    ) -> anyhow::Result<Event> {
        let local_command = command;
        let trace_id = dd.get_trace_id();
        let rv = execute_local_command(local_command.as_ref(), trace_id.clone(), tx.clone())
            .await
            .map(|output| {
                Event::command_finished(local_command.name.clone(), dd.get_trace_id(), output)
            });

        match rv {
            Ok(ref comm) => {
                tx.send(comm.clone()).await.unwrap();
            }
            Err(_) => todo!("Haven't handled the error case yet"),
        }
        Ok(rv?)
    }
}

async fn execute_local_command(
    command: &Command,
    trace_id: String,
    tx_chan: Sender<Event>,
) -> anyhow::Result<CommandOutput> {
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
    } = prepare_workspace(command).await?;
    let mut commandlocal = tokio::process::Command::new("bash");
    commandlocal
        .arg(script_file)
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
