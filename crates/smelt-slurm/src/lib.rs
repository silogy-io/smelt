use std::{path::PathBuf, process::Stdio};

use anyhow::{Result};
use smelt_core::{Command};
use smelt_data::{
    event_listener_client::EventListenerClient,
    executed_tests::{TestOutputs, TestResult},
    Event,
};
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
    script_file: PathBuf,
    trace_id: String,
    host: String,
) -> Result<()> {
    let shell = "bash";
    let mut stream = EventListenerClient::connect(host).await?;
    let _ = stream
        .send_event(Event::command_started(
            command_name.to_string(),
            trace_id.clone(),
        ))
        .await?;
    let stdout = script_file.parent().unwrap().join(Command::stdout_file());

    let mut stdout = File::create(&stdout).await?;

    let mut commandlocal = tokio::process::Command::new(shell);

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
    let _maybe_pid = comm_handle.id();
    let silent = true;

    let cstatus: TestOutputs = loop {
        tokio::select!(
            Ok(Some(line)) = lines.next_line() => {
                handle_line(command_name,line, trace_id.as_str(), &mut stdout, silent,&mut stream).await;
            }
            Ok(Some(line)) = stderr_lines.next_line() => {
                handle_line(command_name,line, trace_id.as_str(), &mut stdout, silent,&mut stream).await;


            }
            status_code = comm_handle.wait() => {
                break status_code.map(|val| TestOutputs{ exit_code: val.code().unwrap_or(-555), artifacts: vec![]});
            }


        );
    }?;

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
    let res = TestResult {
        test_name: command_name.to_string(),
        outputs: Some(cstatus),
    };
    let _ = stream.send_outputs(res).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    
}
