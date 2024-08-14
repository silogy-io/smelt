use crate::Command;

use dice::DiceData;

use smelt_data::{
    executed_tests::{
        artifact_pointer::Pointer, ArtifactPointer, ExecutedTestResult, TestOutputs, TestResult,
    },
    Event,
};

pub use smelt_core::{get_target_root, prepare_workspace, Workspace};
use smelt_events::runtime_support::GetSmeltRoot;
use tokio::{fs::File, io::AsyncWriteExt, sync::mpsc::Sender};

pub(crate) async fn handle_line(
    command: &Command,
    line: String,
    trace_id: String,
    tx_chan: &Sender<Event>,
    stdout: &mut File,
    avoid_message: bool,
) {
    if !avoid_message {
        let _handleme = tx_chan
            .send(Event::command_stdout(
                command.name.clone(),
                trace_id.clone(),
                line.clone(),
            ))
            .await;
    }
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
        pointer: Some(Pointer::Path(format!(
            "{}/command.out",
            get_target_root(smelt_root.to_string_lossy(), &command.name),
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
