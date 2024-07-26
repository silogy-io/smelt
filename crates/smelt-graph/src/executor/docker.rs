use std::{collections::HashMap, sync::Arc};
use anyhow::Error;
use async_trait::async_trait;
use bollard::{container::LogsOptions, Docker};
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    service::HostConfig,
};
use bollard::container::LogOutput;
use bollard::models::ResourcesUlimits;
use dice::{DiceData, UserComputationData};
use futures::StreamExt;
use tokio::fs::File;
use smelt_core::SmeltErr;
use smelt_data::{Event, executed_tests::ExecutedTestResult};
use smelt_data::client_commands::{CfgDocker, RunMode, Ulimit};
use smelt_events::runtime_support::{GetSmeltCfg, GetSmeltRoot, GetTraceId, GetTxChannel};

use crate::Command;
use crate::executor::Executor;

use super::common::{create_test_result, get_target_root, handle_line, prepare_workspace};

pub struct DockerExecutor {
    docker_client: Docker,
    /// Name of the image the docker executor will be using
    image_name: String,
    /// Default mounts for the docker executor
    /// these should be in the form
    additional_mounts: HashMap<String, String>,
    /// Additional flags to pass into the Docker run command
    ulimits: Vec<Ulimit>,
    mac_address: Option<String>,
    run_mode: RunMode,
    artifact_bind_directory: String,
}

impl DockerExecutor {
    pub fn new(
        cfg_docker: &CfgDocker,
    ) -> anyhow::Result<Self> {
        let docker_client = Docker::connect_with_defaults()?;
        let run_mode = match RunMode::from_i32(cfg_docker.run_mode) {
            Some(mode) => mode,
            None => {
                return Err(Error::from(SmeltErr::InvalidConfig { reason: format!("Unknown docker run_mode: {}", cfg_docker.run_mode) }))
            }
        };

        Ok(Self {
            image_name: cfg_docker.image_name.clone(),
            docker_client,
            additional_mounts: cfg_docker.additional_mounts.clone(),
            ulimits: cfg_docker.ulimits.clone(),
            mac_address: cfg_docker.mac_address.clone(),
            run_mode,
            artifact_bind_directory: cfg_docker.artifact_bind_directory.clone(),
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
    ) -> anyhow::Result<ExecutedTestResult> {
        let shell = "bash";
        let trace_id = dd.get_trace_id();
        let tx = dd.get_tx_channel();
        let docker = &self.docker_client;
        let root = global_data.get_smelt_root();
        let root_as_str = root
            .to_str()
            .expect("Smelt root couldnt be converted to string ");

        let command_default_dir = command.working_dir.clone();
        let silent = global_data.get_smelt_cfg().silent;

        // "Prepares" the workspace for this command -- creates a directory at path
        // {SMELT_ROOT}/smelt-out/{COMMAND_NAME}
        let mut stdout = File::open("/dev/null").await?;

        let cmd = match self.run_mode {
            RunMode::Local => {
                let workspace = prepare_workspace(&command, root.clone(), command_default_dir.as_path()).await?;
                stdout = workspace.stdout;
                vec![shell.to_string(), workspace.script_file.to_str().unwrap().to_string()]
            }
            RunMode::Remote => {
                // Create the target root before running any commands
                let mut sub_command = vec![format!("mkdir -p {}", get_target_root(root_as_str, &command.name))];
                sub_command.append(&mut command.script.clone());
                vec![
                    "bash".to_string(),
                    "-c".to_string(),
                    sub_command.join(" && "),
                ]
            }
        };

        // we can derive platform info from inspecting the image, but we don't need to do that
        // let inspect = docker.inspect_image(self.image_name.as_str()).await?;

        let artifact_bind = format!("{}:{}", format!("{}/{}", &self.artifact_bind_directory, &command.name), "/tmp/artifacts/");
        let binds = match self.run_mode {
            RunMode::Local => {
                // The "default" bind mount for all commands is smelt root -- in expectation, this should
                // mount the git root in to the container, at the same path as it has on the host
                // filesystem
                let base_binds = vec![format!("{}:{}", root_as_str, root_as_str), artifact_bind];
                Some(self
                    .additional_mounts
                    .iter()
                    .fold(base_binds, |mut val, b| {
                        val.push(format!("{}:{}", b.0, b.1));
                        val
                    }))
            }
            RunMode::Remote => Some(vec![artifact_bind])
        };

        let ulimits = self.ulimits.iter().map(|ulimit| {
                ResourcesUlimits {
                    name: ulimit.name.clone(),
                    soft: ulimit.soft,
                    hard: ulimit.hard,
                }
        }).collect::<Vec<_>>();

        // Define the container options
        let container_config: Config<String> = Config {
            image: Some(self.image_name.clone()),
            working_dir: Some(root_as_str.to_string()),
            cmd: Some(cmd),
            env: Some(vec![
                format!("SMELT_ROOT={}", root_as_str),
                format!("TARGET_ROOT={}", get_target_root(root_as_str, &command.name)),
            ]),
            mac_address: self.mac_address.clone(),
            host_config: Some(HostConfig {
                binds,
                ulimits: Some(ulimits),
                ..Default::default()
            }),

            ..Default::default()
        };

        // TODO: we will probably need to inspect the image and set the platform for robust macos support
        let platform = None;
        let container_name = command.name.clone();

        let _ = docker.remove_container(container_name.as_ref(), None).await;
        // Create the container
        let container = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name.as_str(),
                    platform,
                }),
                container_config,
            )
            .await?;

        // Send a message that the command has started, and then start the container
        let _ = tx
            .send(Event::command_started(
                command.name.clone(),
                trace_id.clone(),
            ))
            .await;
        docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await?;

        // attach to docker logs -- this will also pick up any output that was emitted between the
        // container being started and the "attaching"
        let attach_options: LogsOptions<String> = LogsOptions {
            stdout: true,
            stderr: true,
            ..LogsOptions::default()
        };
        let mut output = docker.logs(&container.id, Some(attach_options));

        // Stream the stdout and stderr ouput from the docker container via Event messages
        while let Some(message) = output.next().await {
            match message {
                Ok(output) => match output {
                    LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                        if let Ok(line) = String::from_utf8(message.to_vec()) {
                            handle_line(
                                command.as_ref(),
                                line,
                                trace_id.clone(),
                                &tx,
                                &mut stdout,
                                silent,
                            )
                            .await;
                        }
                    }

                    // From looking at the code, console messages are docker telemetry that come
                    // from decoding messages from the docker socket
                    LogOutput::Console { message } => {
                        if let Ok(line) = String::from_utf8(message.to_vec()) {
                            eprintln!("Not handling console output right now: {}", line)
                        }
                    }
                    LogOutput::StdIn { message: _ } => {}
                },
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        // get status code from the container, which should be exited at this point
        let status_code = docker
            .inspect_container(container_name.as_str(), None)
            .await?
            .state
            .and_then(|state| state.exit_code)
            .unwrap_or(1) as i32;
        Ok(create_test_result(
            command.as_ref(),
            status_code,
            global_data,
        ))
    }
}
