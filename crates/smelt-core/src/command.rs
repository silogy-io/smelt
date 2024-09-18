use allocative::Allocative;
use dupe::Dupe;

use serde::{Deserialize, Serialize};

use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::SmeltErr;

use crate::CommandDefPath;

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
    #[serde(default)]
    pub on_failure: Option<CommandDependency>,
}

impl Command {
    /// Name of the shell script that will be executed by each command
    pub const fn script_file() -> &'static str {
        "command.sh"
    }
    /// Name of the json file that is used to track expected artifacts -- only used for the sealed
    /// configuration of the smelt executor
    pub const fn artifacts_json() -> &'static str {
        "artifacts.json"
    }
    /// Name of the file that contains stderr output
    ///
    pub const fn stderr_file() -> &'static str {
        "command.err"
    }

    /// Name of the file that contains stdout output for each command
    pub const fn stdout_file() -> &'static str {
        "command.out"
    }

    pub fn default_target_root<AP: AsRef<Path>>(&self, root: AP) -> Result<PathBuf, SmeltErr> {
        let root = root.as_ref();
        Ok(root.join("smelt-out").join(&self.name))
    }

    pub fn script_contents(&self) -> impl Iterator<Item = String> + '_ {
        self.script.iter().cloned()
    }
}

#[derive(Serialize, Deserialize, Clone, Dupe, PartialEq, Eq, Hash, Debug, Allocative)]
#[serde(rename_all = "lowercase")]
pub enum TargetType {
    Test,
    Stimulus,
    Build,
    Rerun,
    Rebuild,
}

impl TargetType {
    pub fn test_only_valid(&self) -> bool {
        match self {
            Self::Test | Self::Rerun | Self::Rebuild => true,
            _ => false,
        }
    }
}

impl FromStr for TargetType {
    type Err = SmeltErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "test" => Ok(TargetType::Test),
            "stimulus" => Ok(TargetType::Stimulus),
            "build" => Ok(TargetType::Build),
            "rebuild" => Ok(TargetType::Rebuild),
            "rerun" => Ok(TargetType::Rerun),
            _ => Err(SmeltErr::BadTargetType(s.to_string())),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative)]
pub struct Runtime {
    pub num_cpus: u32,
    pub max_memory_mb: u32,
    pub timeout: u32,
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
                TargetType::Rerun => "rerun",
                TargetType::Rebuild => "rebuild",
            }
        )
    }
}

impl fmt::Display for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Runtime {{ num_cpus: {}, max_memory_mb: {}, timeout: {}, }}",
            self.num_cpus, self.max_memory_mb, self.timeout
        )
    }
}

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
