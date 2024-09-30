use anyhow::Error;
use async_trait::async_trait;
use bollard::container::LogOutput;
use bollard::models::ResourcesUlimits;
use bollard::{container::LogsOptions, Docker};
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    errors::Error as BollardError,
    service::HostConfig,
};
use chrono::Utc;
use dice::{DiceData, UserComputationData};
use futures::StreamExt;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::{collections::HashMap, sync::Arc};
use tokio::fs::File;

use smelt_core::SmeltErr;
use smelt_data::client_commands::{CfgDocker, RunMode, Ulimit};
use smelt_data::{executed_tests::ExecutedTestResult, Event};
use smelt_events::runtime_support::{GetSmeltCfg, GetSmeltRoot, GetTraceId, GetTxChannel};

use crate::executor::Executor;
use crate::Command;
use smelt_rt::profile_cmd_docker;

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
    pub fn new(cfg_docker: &CfgDocker) -> anyhow::Result<Self> {
        let docker_client = Docker::connect_with_defaults()?;
        let run_mode = match RunMode::try_from(cfg_docker.run_mode) {
            Ok(mode) => mode,
            Err(_) => {
                return Err(Error::from(SmeltErr::InvalidConfig {
                    reason: format!("Unknown docker run_mode: {}", cfg_docker.run_mode),
                }))
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
                let workspace =
                    prepare_workspace(&command, root.clone(), command_default_dir.as_path())
                        .await?;
                stdout = workspace.stdout;
                vec![
                    shell.to_string(),
                    workspace.script_file.to_str().unwrap().to_string(),
                ]
            }
            RunMode::Remote => {
                // Create the target root before running any commands
                let mut sub_command = vec![format!(
                    "mkdir -p {}",
                    get_target_root(root_as_str, &command.name)
                )];
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

        let artifact_bind = format!(
            "{}:{}",
            format!("{}/{}", &self.artifact_bind_directory, &command.name),
            "/tmp/artifacts/"
        );

        let binds = match self.run_mode {
            RunMode::Local => {
                // The "default" bind mount for all commands is smelt root -- in expectation, this should
                // mount the git root in to the container, at the same path as it has on the host
                // filesystem
                let base_binds = vec![format!("{}:{}", root_as_str, root_as_str), artifact_bind];
                Some(
                    self.additional_mounts
                        .iter()
                        .fold(base_binds, |mut val, b| {
                            val.push(format!("{}:{}", b.0, b.1));
                            val
                        }),
                )
            }
            RunMode::Remote => Some(vec![artifact_bind]),
        };

        let ulimits = self
            .ulimits
            .iter()
            .map(|ulimit| ResourcesUlimits {
                name: ulimit.name.clone(),
                soft: ulimit.soft,
                hard: ulimit.hard,
            })
            .collect::<Vec<_>>();

        // Define the container options
        let container_config: Config<String> = Config {
            image: Some(self.image_name.clone()),
            working_dir: Some(root_as_str.to_string()),
            cmd: Some(cmd),
            env: Some(vec![
                format!("SMELT_ROOT={}", root_as_str),
                format!(
                    "TARGET_ROOT={}",
                    get_target_root(root_as_str, &command.name)
                ),
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
        // 11 * lg(62) = 65.5 bits of randomness. According to
        // https://en.wikipedia.org/wiki/Birthday_problem#Probability_table
        // the probability of a collision for one million samples is less than one in a million
        let suffix_length = 11;
        let container_name_prefix: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(suffix_length)
            .map(char::from)
            .collect();

        let container_name = format!("{}_{}", command.name, container_name_prefix);

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

        let profile_start_time_millis: u64 = Utc::now().timestamp_millis().try_into().unwrap();
        let docker_clone = docker.clone();
        let container_name_clone = container_name.clone();
        let tx_clone = tx.clone();
        let command_name_clone = command.name.clone();
        let trace_id_clone = trace_id.clone();
        let sample_task = tokio::spawn(async move {
            profile_cmd_docker(
                tx_clone,
                docker_clone,
                &container_name_clone,
                command_name_clone,
                trace_id_clone,
                profile_start_time_millis,
            )
            .await;
        });

        let command_clone = command.clone();
        let docker_clone = docker.clone();
        let log_task = tokio::spawn(async move {
            // attach to docker logs -- this will also pick up any output that was emitted between the
            // container being started and the "attaching"
            let attach_options: LogsOptions<String> = LogsOptions {
                stdout: true,
                stderr: true,
                follow: true,
                ..LogsOptions::default()
            };
            let mut output = docker_clone.logs(&container.id, Some(attach_options));
            while let Some(message) = output.next().await {
                match message {
                    Ok(output) => match output {
                        LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                            let line = String::from_utf8_lossy(&message);
                            handle_line(
                                &command_clone,
                                line.to_string(),
                                trace_id.clone(),
                                &tx,
                                &mut stdout,
                                silent,
                            )
                            .await;
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
        });

        // Need to explicitly wait for container to exit. The closing of output is not a reliable
        // signal for the container having exited.
        let status_code = match docker
            .wait_container::<&str>(container_name.as_str(), None)
            .next()
            .await
        {
            Some(Ok(response)) => response.status_code,
            Some(Err(BollardError::DockerContainerWaitError { error: _, code })) => {
                // This is how wait_container returns a non-zero exit code from the container, as
                // well as if waiting for the container returned an error.
                code
            }
            Some(Err(e)) => {
                tracing::error!("Unhandled error from docker wait: {}", e);
                1
            }
            None => {
                tracing::error!("Container {} returned no exit code", container_name);
                1
            }
        };

        log_task.abort();
        sample_task.abort();

        Ok(create_test_result(
            command.as_ref(),
            status_code.try_into().unwrap(),
            global_data,
        ))
    }
}
