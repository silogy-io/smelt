use allocative::Allocative;
use dice::DiceError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::PyErr;

use thiserror::Error;

use crate::CommandDefPath;

#[derive(Error, Debug)]
pub enum SmeltErr {
    #[error("unknown error")]
    Unknown,
    #[error("Dice failure {0}")]
    DiceFail(#[from] DiceError),
    #[error("IoError {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serde yaml error {0}")]
    SerdeYamlError(#[from] serde_yaml::Error),
    #[error("Serde json error {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Command cache miss")]
    CommandCacheMiss,
    #[error("Invalid target type {0}")]
    BadTargetType(String),
    #[error("Executor failed to execute with error : {0}")]
    ExecutorFailed(String),
    #[error("Dependency for a command named {missing_dep_name} was found, but no such command was declared")]
    MissingCommandDependency { missing_dep_name: String },
    #[error("Dependency for a command named {missing_file_name} was found, but no such command was declared")]
    MissingFileDependency { missing_file_name: String },
    #[error("Setting commands failed; reason is {reason}")]
    CommandSettingFailed { reason: String },
    #[error("Two commands with the same name {name} where declared")]
    DuplicateCommandName { name: String },
    #[error("{output} was declared twice!")]
    DuplicateOutput { output: CommandDefPath },
    #[error("The following outputs were declared but never created: {missing_outputs:?}")]
    MissingOutputs {
        missing_outputs: Vec<CommandDefPath>,
    },
    #[error("Artifact name cannot be parsed out")]
    BadArtifactName,
}

impl Allocative for SmeltErr {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut allocative::Visitor<'b>) {
        let vis = visitor.enter_self(&self);
        vis.exit();
    }
}

impl From<SmeltErr> for PyErr {
    fn from(smelt_err: SmeltErr) -> Self {
        let smelt_string = smelt_err.to_string();
        PyRuntimeError::new_err(smelt_string)
    }
}
