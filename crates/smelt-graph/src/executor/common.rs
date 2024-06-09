use std::path::PathBuf;
use std::{io::Write, path::Path};

use crate::Command;

use dice::DiceData;

use smelt_data::{
    executed_tests::{ArtifactPointer, ExecutedTestResult, TestOutputs, TestResult},
    Event,
};

use smelt_events::runtime_support::GetCmdDefPath;
use tokio::{fs::File, io::AsyncWriteExt, sync::mpsc::Sender};

pub(crate) struct Workspace {
    pub(crate) script_file: PathBuf,
    pub(crate) stdout: File,
    pub(crate) working_dir: PathBuf,
}

/// Creates all of the directory scaffolding expected by a command
///
/// This function is currently used across all executors, and is always executed in the host
/// filesystem
pub(crate) async fn prepare_workspace(
    command: &Command,
    smelt_root: PathBuf,
) -> anyhow::Result<Workspace> {
    let env = &command.runtime.env;
    let working_dir = command.default_target_root(smelt_root.as_path())?;
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

pub(crate) fn create_test_result(
    command: &Command,
    exit_code: i32,
    global_data: &DiceData,
) -> ExecutedTestResult {
    let command_default_dir = global_data.get_cmd_def_path();
    let mut missing_artifacts = vec![];
    let mut artifacts = vec![];
    for output in command.outputs.iter() {
        let path = output.to_path(command_default_dir.as_path());
        let path_exists = path.exists();
        let default_name = path
            .file_name()
            .expect("Filename missing from an artifact")
            .to_string_lossy()
            .to_string();
        let artifact = ArtifactPointer::file_artifact(default_name, path);
        if !path_exists {
            missing_artifacts.push(artifact)
        } else {
            artifacts.push(artifact);
        }
    }

    let test_result = TestResult {
        test_name: command.name.clone(),
        outputs: Some(TestOutputs {
            artifacts,
            exit_code,
        }),
    };

    if missing_artifacts.is_empty() {
        ExecutedTestResult::Success(test_result)
    } else {
        ExecutedTestResult::MissingFiles {
            test_result,
            missing_artifacts,
        }
    }
}
