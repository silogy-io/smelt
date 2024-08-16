use std::fmt::Display;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Command;

use tokio::{fs::File, io::AsyncWriteExt};

pub struct Workspace {
    pub script_file: PathBuf,
    pub stdout: File,
}

pub fn get_target_root<T: Display, S: Display>(smelt_root: T, command_name: S) -> String {
    // TODO -- maybe parameterize "smelt-out"?
    format!("{}/{}/{}", smelt_root, "smelt-out", command_name)
}

/// Creates all of the directory scaffolding expected by a command
///
/// This function is currently used across all executors, and is always executed in the host
/// filesystem
pub async fn prepare_workspace(
    command: &Command,
    smelt_root: PathBuf,
    command_working_dir: &Path,
) -> anyhow::Result<Workspace> {
    let working_dir = command.default_target_root(smelt_root.as_path())?;
    let script_file = working_dir.join(Command::script_file());
    let stdout_file = working_dir.join(Command::stdout_file());
    tokio::fs::create_dir_all(&working_dir).await?;
    let mut file = File::create(&script_file).await?;

    let stdout = File::create(&stdout_file).await?;

    let mut buf: Vec<u8> = Vec::new();

    writeln!(buf, "export SMELT_ROOT={}", smelt_root.to_string_lossy())?;

    writeln!(
        buf,
        "export TARGET_ROOT={}",
        get_target_root(smelt_root.to_string_lossy(), &command.name)
    )?;

    writeln!(buf, "cd {}", command_working_dir.to_string_lossy())?;

    for script_line in &command.script {
        writeln!(buf, "{}", script_line)?;
    }

    file.write_all(&buf).await?;
    file.flush().await?;
    Ok(Workspace {
        script_file,
        stdout,
    })
}
