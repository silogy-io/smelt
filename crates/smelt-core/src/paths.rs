use allocative::Allocative;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[repr(transparent)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative, Display)]
pub struct SmeltPath(String);
#[repr(transparent)]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug, Allocative, Display)]
pub struct CommandDefPath(String);

impl SmeltPath {
    pub fn new(path: String) -> Self {
        // TODO! PARSE! MAKE SURE THIS PATH IS VALID!
        Self(path)
    }
    pub fn to_path(&self, smelt_root: &Path) -> PathBuf {
        smelt_root.join(Path::new(self.0.as_str()))
    }
}

impl CommandDefPath {
    pub fn new(path: String) -> Self {
        Self(path)
    }

    pub fn to_path(&self, command_dir_path: &Path) -> PathBuf {
        command_dir_path.join(Path::new(self.0.as_str()))
    }
}
