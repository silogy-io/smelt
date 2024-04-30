use crate::executor::Executor;
use std::io::Write;
use std::{path::PathBuf, sync::Arc};

use crate::Command;
use dice::{DiceData, UserComputationData};
use otl_core::OtlErr;
use otl_data::{
    command_event::CommandVariant, CommandEvent, CommandFinished, CommandOutput, Event,
};
use otl_events::{runtime_support::GetTraceId, to_file};

use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::mpsc::{channel, Sender},
};

use super::{ExecutorErr, ExecutorShim};

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
        //let rt = thread::spawn(move || {
        //    let rt = tokio::runtime::Builder::new_multi_thread()
        //        .worker_threads(self.threads)
        //        .build()
        //        .unwrap();
        //    rt
        //})
        //.join()
        //.unwrap();

        Ok(LocalExecutor {})
    }
}

pub struct LocalExecutor {}

impl Executor for LocalExecutor {
    async fn execute_commands(
        &self,
        command: Arc<Command>,
        tx: Sender<Event>,
        dd: &UserComputationData,
    ) -> Result<Event, ExecutorErr> {
        let val = dd.get_trace_id();
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

impl From<LocalExecutor> for ExecutorShim {
    fn from(val: LocalExecutor) -> Self {
        ExecutorShim::Local(val)
    }
}

async fn execute_local_command(
    command: &Command,
    trace_id: String,
    _tx_chan: Sender<Event>,
) -> Result<CommandOutput, std::io::Error> {
    let env = &command.runtime.env;
    let working_dir = env
        .get("TARGET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| command.default_target_root().unwrap());

    let script_file = working_dir.join(Command::script_file());
    let stderr_file = working_dir.join(Command::stderr_file());
    let stdout_file = working_dir.join(Command::stdout_file());
    tokio::fs::create_dir_all(&working_dir).await?;
    let mut file = File::create(&script_file).await?;
    let stderr = File::create(&stderr_file).await?;
    let stdout = File::create(&stdout_file).await?;

    let mut buf: Vec<u8> = Vec::new();

    for (env_name, env_val) in env.iter() {
        writeln!(buf, "export {}={}", env_name, env_val)?;
    }

    for script_line in &command.script {
        writeln!(buf, "{}", script_line)?;
    }

    file.write_all(&mut buf).await?;
    file.flush().await?;

    _tx_chan
        .send(Event::command_started(command.name.clone(), trace_id))
        .await;
    let mut command = tokio::process::Command::new("bash");
    command
        .arg(script_file)
        .stdout(stdout.into_std().await)
        .stderr(stderr.into_std().await);
    let cstsatus = command.status().await.map(|val| CommandOutput {
        status_code: val.code().unwrap_or(-555),
    })?;

    to_file(&cstsatus, &working_dir).await?;
    Ok(cstsatus)
}
