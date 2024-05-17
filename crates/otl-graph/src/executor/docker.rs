use crate::executor::Executor;
use crate::Command;
use async_trait::async_trait;
use dice::{DiceData, UserComputationData};
use futures::StreamExt;

use otl_data::{CommandOutput, Event};

use otl_events::runtime_support::{GetOtlRoot, GetTraceId, GetTxChannel};
use std::{collections::HashMap, sync::Arc};

use bollard::container::{InspectContainerOptions, LogOutput};
use bollard::{container::LogsOptions, Docker};
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    service::HostConfig,
};

use super::common::{handle_line, prepare_workspace, Workspace};

pub struct DockerExecutor {
    docker_client: Docker,
    /// Name of the image the docker executor will be using
    image_name: String,
    /// Default mounts for the docker executor
    /// these should be in the form
    additional_mounts: HashMap<String, String>,
}

impl DockerExecutor {
    pub fn new(
        image_name: String,
        additional_mounts: HashMap<String, String>,
    ) -> anyhow::Result<Self> {
        let docker_client = Docker::connect_with_socket_defaults()?;

        Ok(Self {
            image_name,
            docker_client,
            additional_mounts,
        })
    }
}

#[async_trait]
impl Executor for DockerExecutor {
    async fn execute_commands(
        &self,
        command: Arc<Command>,
        dd: &UserComputationData,
        global_data: &DiceData,
    ) -> anyhow::Result<Event> {
        let shell = "bash";
        let trace_id = dd.get_trace_id();

        let docker = Docker::connect_with_local_defaults()?;
        let root = global_data.get_otl_root();
        let root_as_str = root
            .to_str()
            .expect("Otl root couldnt be converted to string ")
            .to_string();

        let Workspace {
            script_file,
            mut stdout,
            working_dir: _,
        } = prepare_workspace(&command, root.clone()).await?;

        let base_binds = vec![format!("{}:{}", root_as_str, root_as_str)];
        let binds = self
            .additional_mounts
            .iter()
            .fold(base_binds, |mut val, b| {
                val.push(format!("{}:{}", b.0, b.1));
                val
            });
        let cmd = vec![shell.to_string(), script_file.to_str().unwrap().to_string()];

        // we can derive platform info from inspecting the image, but we don't need to do that
        // let inspect = docker.inspect_image(self.image_name.as_str()).await?;

        let binds = if binds.is_empty() { Some(binds) } else { None };
        // Define the container options
        let container_config: Config<String> = Config {
            image: Some(self.image_name.clone()),
            working_dir: Some(root_as_str),
            cmd: Some(cmd),
            host_config: Some(HostConfig {
                binds,
                ..Default::default()
            }),

            ..Default::default()
        };

        #[cfg(target_os = "macos")]
        let platform = None;

        #[cfg(not(target_os = "macos"))]
        let platform = None;

        let _ = docker.remove_container(command.name.as_ref(), None).await;
        // Create the container
        let container = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: command.name.clone(),
                    platform,
                }),
                container_config,
            )
            .await?;

        let tx = global_data.get_tx_channel();
        let _ = tx
            .send(Event::command_started(
                command.name.clone(),
                trace_id.clone(),
            ))
            .await;

        // Start the container
        docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await?;

        // Attach to the container
        let attach_options: LogsOptions<String> = LogsOptions {
            stdout: true,
            stderr: true,
            ..LogsOptions::default()
        };

        let mut output = docker.logs(&container.id, Some(attach_options));

        // Stream the stdout and stderr
        while let Some(message) = output.next().await {
            match message {
                Ok(output) => match output {
                    LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                        if let Ok(line) = String::from_utf8(message.to_vec()) {
                            handle_line(command.as_ref(), line, trace_id.clone(), &tx, &mut stdout)
                                .await;
                        }
                    }

                    LogOutput::Console { message } => {
                        if let Ok(line) = String::from_utf8(message.to_vec()) {
                            eprintln!("Not handling console output right now: {}", line)
                        }
                    }
                    LogOutput::StdIn { message } => {
                        if let Ok(line) = String::from_utf8(message.to_vec()) {
                            eprintln!("Not handling console output right now: {}", line)
                        }
                    }
                },
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        let dummy_output = CommandOutput { status_code: 0 };
        let done = Event::command_finished(command.name.clone(), dd.get_trace_id(), dummy_output);
        let _ = tx.send(done.clone()).await;
        Ok(done)
    }
}
