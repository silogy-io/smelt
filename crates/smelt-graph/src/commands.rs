use allocative::Allocative;
use dupe::Dupe;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use smelt_core::SmeltErr;

use crate::digest::{CommandDefDigest, CommandIdDigest};
use smelt_core::CommandDefPath;

#[repr(transparent)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct CommandDependency(String);

impl CommandDependency {
    pub fn get_command_name(&self) -> &str {
        if self.0.starts_with("//") {
            self.0.split_once(':').unwrap().1
        } else {
            self.0.as_str()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct Command {
    pub name: String,
    pub target_type: TargetType,
    pub script: Vec<String>,
    #[serde(default)]
    pub dependent_files: Vec<CommandDefPath>,
    #[serde(default)]
    pub dependencies: Vec<CommandDependency>,
    #[serde(default)]
    pub outputs: Vec<CommandDefPath>,
    pub runtime: Runtime,
    #[serde(default)]
    pub working_dir: PathBuf,
}

impl Command {
    pub fn def_digest(&self) -> CommandDefDigest {
        let mut hasher = Sha1::new();
        hasher.update(&self.name);
        hasher.update(&self.target_type.to_string());
        for line in self.script.iter() {
            hasher.update(line);
        }
        for dep in self.dependencies.iter() {
            hasher.update(&dep.0);
        }

        let rv: [u8; 20] = hasher.finalize().into();
        CommandDefDigest::new(rv)
    }
    pub fn id_digest(&self) -> CommandIdDigest {
        let mut hasher = Sha1::new();
        hasher.update(&self.name);
        let rv: [u8; 20] = hasher.finalize().into();
        CommandIdDigest::new(rv)
    }

    pub const fn script_file() -> &'static str {
        "command.sh"
    }

    pub const fn stderr_file() -> &'static str {
        "command.err"
    }

    pub const fn stdout_file() -> &'static str {
        "command.out"
    }

    pub fn default_target_root(&self, root: &Path) -> Result<PathBuf, SmeltErr> {
        Ok(root.join("smelt-out").join(&self.name))
    }

    pub fn script_contents(&self) -> impl Iterator<Item = String> + '_ {
        self.runtime
            .env
            .iter()
            .map(|(env_name, env_val)| format!("export {}={}", env_name, env_val))
            .chain(self.script.iter().cloned())
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
    type Err = SmeltErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "test" => Ok(TargetType::Test),
            "stimulus" => Ok(TargetType::Stimulus),
            "build" => Ok(TargetType::Build),
            _ => Err(SmeltErr::BadTargetType(s.to_string())),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct Runtime {
    pub num_cpus: u32,
    pub max_memory_mb: u32,
    pub timeout: u32,
    pub env: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub command_run_dir: Option<String>,
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

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct CommandScript(Arc<CommandScriptInner>);

#[derive(Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub(crate) struct CommandScriptInner {
    script: Vec<String>,
    deps: Vec<CommandScript>,
}

//pub async fn maybe_cache(command: &Command) -> Result<CommandOutput, SmeltErr> {
//    if let Ok(command_out) = command.get_status_from_fs().await {
//        if command_out.passed() {
//            return Ok(command_out);
//        }
//    } else {
//        //pass
//    };
//
//    execute_command(command).await
//}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deser_simple_yaml() {
        let yaml_data = include_str!("../../../examples/tests_only.smelt.yaml");
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data);

        let _script = script.unwrap();
    }
}
