use crate::executor::Executor;
use crate::Command;
use async_trait::async_trait;
use dice::UserComputationData;
use futures::StreamExt;
use otl_core::OtlErr;
use otl_data::{CommandOutput, Event};
use otl_events::{
    runtime_support::{GetOtlRoot, GetTraceId},
    to_file,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tokio::sync::mpsc::Sender;

use bollard::image::ListImagesOptions;
use bollard::Docker;
use bollard::{
    container::{AttachContainerOptions, Config, CreateContainerOptions, StartContainerOptions},
    service::HostConfig,
};

pub struct DockerExecutor {
    docker_client: Docker,
    /// Name of the image the docker executor will be using
    image_name: String,
    /// Default mounts for the docker executor
    /// these should be in the form
    mounts: BTreeMap<String, String>,
}

impl DockerExecutor {
    fn new(image_name: String) -> anyhow::Result<Self> {
        let docker_client = Docker::connect_with_socket_defaults()?;

        Ok(Self {
            image_name,
            docker_client,
            mounts: BTreeMap::new(),
        })
    }
}

#[async_trait]
impl Executor for DockerExecutor {
    async fn execute_commands(
        &self,
        command: Arc<Command>,
        tx: Sender<Event>,
        dd: &UserComputationData,
    ) -> anyhow::Result<Event> {
        let local_command = command;
        let trace_id = dd.get_trace_id();

        let docker = Docker::connect_with_local_defaults().unwrap();
        let root = dd.get_otl_root();

        let binds = self.mounts.into_iter().fold(Vec::new(), |mut val, b| {
            val.push(format!("{}:{}", b.0, b.1));
            val
        });
        let binds = if binds.is_empty() { Some(binds) } else { None };
        // Define the container options
        let container_config: Config<String> = Config {
            image: Some(self.image_name.clone()),
            host_config: Some(HostConfig {
                binds,
                ..Default::default()
            }),

            ..Default::default()
        };

        #[cfg(target_os = "macos")]
        let platform = Some("linux/amd64".to_string());

        #[cfg(not(target_os = "macos"))]
        let platform = None;

        // Create the container
        let container = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: command.name,
                    platform,
                }),
                container_config,
            )
            .await
            .unwrap();

        // Start the container
        docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .unwrap();

        // Attach to the container
        let mut attach_options = AttachContainerOptions {
            stream: Some(true),
            stdout: Some(true),
            stderr: Some(true),
            ..AttachContainerOptions::default()
        };

        let mut output = docker
            .attach_container(&container.id, Some(attach_options))
            .await
            .unwrap();

        // Stream the stdout and stderr
        while let Some(message) = output.output.next().await {
            match message {
                Ok(output) => println!("{}", &output.to_string()),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }
}
