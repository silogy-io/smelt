use std::future::Future;
use std::pin::Pin;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
};

mod aws;
use aws::upload_file;
pub use aws::AwsCreds;

use anyhow::Result;
use smelt_core::Command;
use smelt_data::{
    event_listener_client::EventListenerClient,
    executed_tests::{TestOutputs, TestResult},
    Event,
};
use smelt_rt::profile_cmd;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};
use tonic::transport::Channel;

pub(crate) async fn handle_line(
    command_name: &str,
    line: String,
    trace_id: &str,
    stdout: &mut File,
    avoid_message: bool,
    tx_chan: &mut EventListenerClient<Channel>,
) {
    if !avoid_message {
        let _handleme = tx_chan
            .send_event(Event::command_stdout(
                command_name.to_string(),
                trace_id.to_string(),
                line.clone(),
            ))
            .await;
    }
    let bytes = line.as_str();
    let _unhandled = stdout.write(bytes.as_bytes()).await;
    let _unhandled = stdout.write(&[b'\n']).await;
}

pub async fn execute_command(
    command_name: &str,
    working_dir: PathBuf,
    trace_id: String,
    host: String,
    maybe_creds: Option<AwsCreds>,
) -> Result<()> {
    let shell = "bash";
    let mut stream = EventListenerClient::connect(host).await?;
    let _ = stream
        .send_event(Event::command_started(
            command_name.to_string(),
            trace_id.clone(),
        ))
        .await?;
    let stdout = working_dir.join(Command::stdout_file());
    let script_file = working_dir.join(Command::script_file());

    let mut stdout = File::create(&stdout).await?;

    let mut commandlocal = tokio::process::Command::new(shell);

    commandlocal
        .arg(script_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut comm_handle = commandlocal.spawn()?;
    let maybe_pid = comm_handle.id();

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    //TODO: maybe nudge this lower or higher
    // currently we are sampling 300 ms
    let freq = 300;
    let sample_task = maybe_pid.map(|pid| {
        tokio::spawn(profile_cmd(
            pid,
            tx.clone(),
            freq,
            command_name.to_string(),
            trace_id.clone(),
        ))
    });

    let stderr = comm_handle.stderr.take().unwrap();
    let stderr_reader = BufReader::new(stderr);
    let mut stderr_lines = stderr_reader.lines();

    let reader = BufReader::new(comm_handle.stdout.take().unwrap());
    let mut lines = reader.lines();
    let _maybe_pid = comm_handle.id();
    // TODO -- parameterize this
    let silent = false;

    // This is the "control loop" for our runtime
    let cstatus: TestOutputs = loop {
        tokio::select!(
            Ok(Some(line)) = lines.next_line() => {
                handle_line(command_name,line, trace_id.as_str(), &mut stdout, silent,&mut stream).await;
            }
            Ok(Some(line)) = stderr_lines.next_line() => {
                handle_line(command_name,line, trace_id.as_str(), &mut stdout, silent,&mut stream).await;
            }
            Some(message) = rx.recv() => {
                let _mayberr = stream.send_event(message).await;
            }
            status_code = comm_handle.wait() => {
                break status_code.map(|val| TestOutputs{ exit_code: val.code().unwrap_or(-555), artifacts: vec![]});
            }
        );
    }?;

    // drains any remaining stdout or stderr output
    while let Ok(Some(line)) = lines.next_line().await {
        handle_line(
            command_name,
            line,
            trace_id.as_str(),
            &mut stdout,
            silent,
            &mut stream,
        )
        .await;
    }

    while let Ok(Some(line)) = stderr_lines.next_line().await {
        handle_line(
            command_name,
            line,
            trace_id.as_str(),
            &mut stdout,
            silent,
            &mut stream,
        )
        .await;
    }

    let res = TestResult {
        test_name: command_name.to_string(),
        outputs: Some(cstatus),
    };

    let _ = stream
        .send_event(Event::command_finished(
            res,
            "test".to_string(),
            trace_id.clone(),
        ))
        .await;

    if let Some(task) = sample_task {
        task.abort()
    }

    if let Some(awscreds) = maybe_creds {
        let bucket = awscreds.bucket.clone();
        let upload =
            handle_artifacts(command_name, working_dir.as_path(), awscreds, &mut stream).await;
        if let Err(err) = upload {
            let _ = stream
                .send_event(Event::runtime_warn(
                    format!("Could not succesfully upload artifacts to s3 due to {err}"),
                    trace_id,
                ))
                .await;
        } else if let Ok(files) = upload {
            let _ = stream
                .send_event(Event::runtime_warn(
                    format!(
                        "Successfully uploaded artifacts to bucket {} at paths {:?}",
                        bucket, files
                    ),
                    trace_id,
                ))
                .await;
        }
    }

    Ok(())
}

fn default_artifacts(working_dir: &Path) -> HashMap<String, String> {
    HashMap::from([(
        String::from("smelt_log"),
        working_dir
            .join("command.out")
            .to_string_lossy()
            .to_string(),
    )])
}

/// Adds artifacts under /tmp/artifacts
fn add_artifact_paths(
    mut artifact_map: HashMap<String, String>,
    artifacts_dir: String,
) -> Pin<Box<dyn Future<Output = std::io::Result<HashMap<String, String>>>>> {
    Box::pin(async move {
        // Create a recursive directory walker
        let mut paths = tokio::fs::read_dir(artifacts_dir).await?;

        // Process each entry in the directory
        while let Some(entry) = paths.next_entry().await? {
            let path = entry.path();

            // Skip if it's a directory - we'll handle its contents separately
            if path.is_dir() {
                // Recursively process subdirectories
                artifact_map =
                    add_artifact_paths(artifact_map, path.to_str().unwrap().to_string()).await?;
                continue;
            }

            // Convert path to string, skip if conversion fails
            if let Some(path_str) = path.to_str() {
                artifact_map.insert(path_str.to_string(), path_str.to_string());
            }
        }

        Ok(artifact_map)
    })
}

/// Uploads all of the visible artifacts
pub(crate) async fn handle_artifacts(
    command_name: &str,
    working_dir: &Path,
    creds: AwsCreds,
    stream: &mut EventListenerClient<Channel>,
) -> anyhow::Result<Vec<String>> {
    let artifact_json = working_dir.join(Command::artifacts_json());
    // TODO Get rid of artifact_json entirely; it adds unnecessary complexity.
    //   All artifacts should be handled by simply putting them in /tmp/artifacts.
    let mut artifact_map: HashMap<String, String> = tokio::fs::read(artifact_json)
        .await
        .map(|bytes| serde_json::from_slice(&bytes).unwrap_or(default_artifacts(working_dir)))
        .inspect_err(|e| println!("failed to deserialize artifact json with err {e}"))
        .unwrap_or(default_artifacts(working_dir));

    artifact_map = add_artifact_paths(artifact_map.clone(), String::from("/tmp/artifacts"))
        .await
        .unwrap_or_else(|e| {
            println!("Failed to scan artifact directory: {e}");
            artifact_map
        });

    let client = aws::create_s3_client(&creds).await?;
    let mut artifacts = vec![];
    for artifact in artifact_map.values() {
        let artifactpb = PathBuf::from(artifact);
        if tokio::fs::metadata(&artifactpb)
            .await
            .is_ok_and(|val| val.is_file())
        {
            let upload_path = upload_file(command_name, &client, &creds, artifactpb)
                .await
                .inspect_err(|_e| {
                    println!("Failed to upload artifact to s3 at path {artifact} with err {_e}")
                })
                .inspect(|_| println!("Sucessfully uploaded {artifact:?}"));
            if let Ok(path) = upload_path {
                artifacts.push(path);
            } else if let Err(e) = upload_path {
                let _ = stream
                    .send_event(Event::runtime_warn(
                        format!("Failed to upload artifact to s3 at path {artifact} with err {e}"),
                        "TESTINGONLY".to_string(),
                    ))
                    .await;
            }
        }
    }

    Ok(artifacts)
}
