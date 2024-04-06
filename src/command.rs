use serde::{Deserialize, Serialize};

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;

use std::{fmt, path::Path, sync::Arc};

use tokio::{fs::File, io::AsyncWriteExt};

use crate::error::OtlErr;
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct Command {
    pub name: String,
    pub target_type: TargetType,
    pub script: Vec<String>,
    pub dependencies: Vec<String>,
    pub outputs: Vec<String>,
    pub runtime: Runtime,
}

#[derive(Serialize, Deserialize, Clone, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
#[serde(rename_all = "lowercase")]
pub enum TargetType {
    Test,
    Stimulus,
    Build,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct Runtime {
    pub num_cpus: u32,
    pub max_memory_mb: u32,
    pub timeout: u32,
    pub env: std::collections::BTreeMap<String, String>,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Command {{ name: {}, target_type: {}, script: {:?}, dependencies: {:?}, outputs: {:?}, runtime: {} }}", 
            self.name, self.target_type, self.script, self.dependencies, self.outputs, self.runtime)
    }
}

impl fmt::Display for TargetType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TargetType::Test => "test",
                TargetType::Stimulus => "stimulus",
                TargetType::Build => "build",
            }
        )
    }
}

impl fmt::Display for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Runtime {{ num_cpus: {}, max_memory_mb: {}, timeout: {}, env: {:?} }}",
            self.num_cpus, self.max_memory_mb, self.timeout, self.env
        )
    }
}

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative)]
pub struct CommandOutput {
    status_code: i32,
}

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct CommandScript(Arc<CommandScriptInner>);

#[derive(Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub(crate) struct CommandScriptInner {
    script: Vec<String>,
    deps: Vec<CommandScript>,
}

pub async fn execute_command(command: &Command) -> Result<CommandOutput, OtlErr> {
    let env = &command.runtime.env;
    let working_dir = Path::new(&env["TARGET_ROOT"]);
    let script_file = working_dir.join("command.sh");
    let stderr_file = working_dir.join("command.err");
    let stdout_file = working_dir.join("command.out");
    tokio::fs::create_dir_all(&working_dir).await?;
    let mut file = File::create(&script_file).await?;
    let stderr = File::create(&stderr_file).await?;
    let stdout = File::create(&stdout_file).await?;

    let mut buf: Vec<u8> = Vec::new();

    use std::io::Write;
    for (env_name, env_val) in env.iter() {
        writeln!(buf, "export {}={}", env_name, env_val)?;
    }

    for script_line in &command.script {
        writeln!(buf, "{}", script_line)?;
    }

    file.write_all(&mut buf).await?;
    file.flush().await?;

    let mut command = tokio::process::Command::new("bash");
    command
        .arg("-C")
        .arg(script_file)
        .stdout(stdout.into_std().await)
        .stderr(stderr.into_std().await);
    Ok(command.status().await.map(|val| CommandOutput {
        status_code: val.code().unwrap_or(-555),
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    #[test]
    fn deser_simple_yaml() {
        let yaml_data = include_str!("../examples/tests_only.otl.yaml");
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data);

        let _script = script.unwrap();
    }
}
