use crate::Command;
use dice::DiceData;
use regex::Regex;
use std::io::{BufRead, BufReader as SyncBufReader};
use tokio::io::AsyncBufReadExt;

pub static STDOUT_LOG: &str = "smelt_log";
use smelt_data::{
    executed_tests::{
        artifact_pointer::Pointer, ArtifactPointer, ExecutedTestResult, TestOutputs, TestResult,
    },
    Event,
};

pub use smelt_core::{get_target_root, prepare_workspace, Workspace};
use smelt_events::runtime_support::GetSmeltRoot;
use tokio::{
    fs::File,
    io::{AsyncWriteExt},
    sync::mpsc::Sender,
};

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
        artifact_name: STDOUT_LOG.into(),
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

    let failure_message = default_extract_failure_message(&artifacts).unwrap_or_default();
    let test_result = TestResult {
        test_name: command.name.clone(),
        outputs: Some(TestOutputs {
            artifacts,
            exit_code,
            failure_message,
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

fn default_extract_failure_message(artifacts: &Vec<ArtifactPointer>) -> Option<String> {
    let maybe_log = artifacts
        .iter()
        .find(|ptr| ptr.artifact_name.as_str() == STDOUT_LOG);
    if let Some(stdout_log) = maybe_log {
        match stdout_log.pointer.as_ref().unwrap() {
            Pointer::Path(path) => {
                let file = std::fs::File::open(path).ok()?;
                let reader = SyncBufReader::new(file);

                let last_line = reader.lines().last();
                last_line.and_then(|val| val.ok())
            }
        }

        //TODO: Make this much more rigorous -- we should extract UVM failures, verilog failures,
        //
    } else {
        // Could not find smelt log -- honestly, this should never happen, so i'll place a tracing
        // warn here
        //
        tracing::warn!("Failed to find stdout log to extract failure message!");
        Some(String::default())
    }
}
fn extract_signature(input: impl ToString) -> String {
    let mut output = input.to_string();

    // Remove non-alpha characters
    //
    // Have to do something a little ugly here
    //

    let re = Regex::new(r"[^a-zA-Z\s\{\}/.]").unwrap();

    let words: Vec<&str> = output.split_whitespace().collect();
    let mut result = Vec::new();

    for word in words {
        if word.contains('/') || word.contains('.') {
            result.push(word.to_string());
        } else {
            result.push(re.replace_all(word, "").to_string());
        }
    }

    // Remove timestamp
    let re = Regex::new(r"\b\d+\b").unwrap();
    output = re.replace_all(&output, "[---").to_string();

    // Remove hierarchies
    let re = Regex::new(r"\b[a-zA-Z]+\.[a-zA-Z]+\b").unwrap();
    output = re.replace_all(&output, "[HIERARCHY]").to_string();

    // Anonymize paths to files
    let re = Regex::new(r"/.+:.+\b").unwrap();
    output = re.replace_all(&output, "[PATH]").to_string();

    output
}
