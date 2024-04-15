use serde::{Deserialize, Serialize};

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;

use std::{fmt, path::PathBuf, str::FromStr, sync::Arc};

use tokio::{fs::File, io::AsyncWriteExt};

use otl_core::OtlErr;
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct Command {
    pub name: String,
    pub target_type: TargetType,
    pub script: Vec<String>,
    pub dependencies: Vec<String>,
    pub outputs: Vec<String>,
    pub runtime: Runtime,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub enum CommandRtStatus {
    /// This command, nor its dependencies, have started running
    Unscheduled,
    //
    Scheduled {
        scheduled_time: std::time::Instant,
    },
    Running {
        scheduled_time: std::time::Instant,
        started_time: std::time::Instant,
    },
    Finished {
        scheduled_time: std::time::Instant,
        started_time: std::time::Instant,
        finished_time: std::time::Instant,
    },
}

impl Command {
    const fn script_file() -> &'static str {
        "command.sh"
    }

    const fn stderr_file() -> &'static str {
        "command.err"
    }

    const fn stdout_file() -> &'static str {
        "command.out"
    }

    fn default_target_root(&self) -> Result<PathBuf, OtlErr> {
        Ok(std::env::current_dir().map(|val| val.join("otl-out").join(&self.name))?)
    }

    pub fn script_contents(&self) -> impl Iterator<Item = String> + '_ {
        self.runtime
            .env
            .iter()
            .map(|(env_name, env_val)| format!("export {}={}", env_name, env_val))
            .chain(self.script.iter().cloned())
    }

    fn working_dir(&self) -> Result<PathBuf, OtlErr> {
        let env = &self.runtime.env;
        let working_dir = env
            .get("TARGET_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.default_target_root().unwrap());
        Ok(working_dir)
    }

    async fn get_status_from_fs(&self) -> Result<CommandOutput, OtlErr> {
        if let Ok(working_dir) = self.working_dir() {
            let val = working_dir
                .exists()
                .then(|| working_dir.join("command.status"));
            if let Some(ile) = val {
                let val: CommandOutput = tokio::fs::read_to_string(ile)
                    .await
                    .map(|val| serde_json::from_str(val.as_str()))??;
                Ok(val)
            } else {
                Err(OtlErr::CommandCacheMiss)
            }
        } else {
            Err(OtlErr::CommandCacheMiss)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
#[serde(rename_all = "lowercase")]
pub enum TargetType {
    Test,
    Stimulus,
    Build,
}

impl FromStr for TargetType {
    type Err = OtlErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "test" => Ok(TargetType::Test),
            "stimulus" => Ok(TargetType::Stimulus),
            "build" => Ok(TargetType::Build),
            _ => Err(OtlErr::BadTargetType(s.to_string())),
        }
    }
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

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative, Serialize, Deserialize)]
pub struct CommandOutput {
    pub(crate) status_code: i32,
}

impl CommandOutput {
    fn passed(&self) -> bool {
        self.status_code == 0
    }

    const fn asfile() -> &'static str {
        "command.status"
    }
    async fn to_file(&self, _base_path: &PathBuf) -> Result<(), OtlErr> {
        let mut command_out = File::create(CommandOutput::asfile()).await?;

        command_out
            .write(serde_json::to_vec(self)?.as_slice())
            .await?;
        Ok(())
    }
}

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct CommandScript(Arc<CommandScriptInner>);

#[derive(Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub(crate) struct CommandScriptInner {
    script: Vec<String>,
    deps: Vec<CommandScript>,
}

pub async fn maybe_cache(command: &Command) -> Result<CommandOutput, OtlErr> {
    if let Ok(command_out) = command.get_status_from_fs().await {
        if command_out.passed() {
            return Ok(command_out);
        }
    } else {
        //pass
    };

    execute_command(command).await
}

pub async fn execute_command(command: &Command) -> Result<CommandOutput, OtlErr> {
    let env = &command.runtime.env;
    let working_dir = env
        .get("TARGET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| command.default_target_root().unwrap());

    let script_file = working_dir.join(Command::script_file());
    let stderr_file = working_dir.join(Command::stderr_file());
    let stdout_file = working_dir.join(Command::stdout_file());
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
    let cstsatus = command.status().await.map(|val| CommandOutput {
        status_code: val.code().unwrap_or(-555),
    })?;
    cstsatus.to_file(&working_dir).await?;
    Ok(cstsatus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deser_simple_yaml() {
        let yaml_data = include_str!("../../../examples/tests_only.otl.yaml");
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data);

        let _script = script.unwrap();
    }
}
