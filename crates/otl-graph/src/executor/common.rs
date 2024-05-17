
use std::{io::Write};
use std::{path::PathBuf};

use crate::Command;



use otl_data::{Event};

use tokio::{
    fs::File,
    io::{AsyncWriteExt},
    sync::mpsc::Sender,
};

pub(crate) struct Workspace {
    pub(crate) script_file: PathBuf,
    pub(crate) stdout: File,
    pub(crate) working_dir: PathBuf,
}

pub(crate) async fn prepare_workspace(
    command: &Command,
    otl_root: PathBuf,
) -> anyhow::Result<Workspace> {
    let env = &command.runtime.env;
    let working_dir = command.default_target_root(otl_root.as_path())?;
    let script_file = working_dir.join(Command::script_file());
    let stdout_file = working_dir.join(Command::stdout_file());
    tokio::fs::create_dir_all(&working_dir).await?;
    let mut file = File::create(&script_file).await?;

    let stdout = File::create(&stdout_file).await?;

    let mut buf: Vec<u8> = Vec::new();

    for (env_name, env_val) in env.iter() {
        writeln!(buf, "export {}={}", env_name, env_val)?;
    }

    for script_line in &command.script {
        writeln!(buf, "{}", script_line)?;
    }

    file.write_all(&buf).await?;
    file.flush().await?;
    Ok(Workspace {
        script_file,
        stdout,
        working_dir,
    })
}

pub(crate) async fn handle_line(
    command: &Command,
    line: String,
    trace_id: String,
    tx_chan: &Sender<Event>,
    stdout: &mut File,
) {
    let _handleme = tx_chan
        .send(Event::command_stdout(
            command.name.clone(),
            trace_id.clone(),
            line.clone(),
        ))
        .await;
    let bytes = line.as_str();
    let _unhandled = stdout.write(bytes.as_bytes()).await;
    let _unhandled = stdout.write(&[b'\n']).await;
}
