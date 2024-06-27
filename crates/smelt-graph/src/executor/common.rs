use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Command;

use dice::DiceData;

use smelt_data::{
    executed_tests::{
        artifact_pointer::Pointer, ArtifactPointer, ExecutedTestResult, TestOutputs, TestResult,
    },
    Event,
};

use smelt_events::runtime_support::GetSmeltRoot;
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
    command_working_dir: &Path,
) -> anyhow::Result<Workspace> {
    // TODO -- maybe parameterize?
    let smeltoutdir = "smelt-out";
    let env = &command.runtime.env;
    let working_dir = command.default_target_root(smelt_root.as_path())?;
    let script_file = working_dir.join(Command::script_file());
    let stdout_file = working_dir.join(Command::stdout_file());
    tokio::fs::create_dir_all(&working_dir).await?;
    let mut file = File::create(&script_file).await?;

    let stdout = File::create(&stdout_file).await?;

    let mut buf: Vec<u8> = Vec::new();

    writeln!(buf, "export SMELT_ROOT={}", smelt_root.to_string_lossy());

    writeln!(
        buf,
        "export TARGET_ROOT={}/{}/{}",
        smelt_root.to_string_lossy(),
        smeltoutdir,
        command.name
    );

    for (env_name, env_val) in env.iter() {
        writeln!(buf, "export {}={}", env_name, env_val)?;
    }

    writeln!(buf, "cd {}", command_working_dir.to_string_lossy());

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
    let command_default_dir = command.working_dir.clone();
    let smelt_root = global_data.get_smelt_root();
    let mut missing_artifacts = vec![];
    let mut artifacts = vec![ArtifactPointer {
        artifact_name: "smelt_log".into(),
        //TODO: smelt-out shouldn't be hardcoded here, sorry for sinning mom
        pointer: Some(Pointer::Path(format!(
            "{}/smelt-out/{}/command.out",
            smelt_root.to_string_lossy().to_string(),
            command.name,
        ))),
    }];

    for output in command.outputs.iter() {
        let path = output.to_path(command_default_dir.as_path(), smelt_root.as_path());
        let path_exists = path.exists();
        let default_name = path
            .file_name()
            .expect("Filename missing from an artifact")
            .to_string_lossy()
            .to_string();
        let artifact = ArtifactPointer::file_artifact(default_name, path);
        if !path_exists {
            tracing::debug!(
                "Missing artifact {:?} for command {}",
                artifact,
                command.name
            );
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
