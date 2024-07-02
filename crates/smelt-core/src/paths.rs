use allocative::Allocative;
use derive_more::Display;
use regex::Regex;
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
        let as_path = Path::new(self.0.as_str());
        if as_path.is_absolute() {
            return as_path.to_path_buf();
        }
        smelt_root.join(as_path)
    }
}

impl CommandDefPath {
    pub fn new(path: String) -> Self {
        Self(path)
    }

    pub fn to_path(&self, command_dir_path: &Path, smelt_root: &Path) -> PathBuf {
        let val = replace_smelt_root(
            self.0.as_str(),
            smelt_root.to_string_lossy().to_string().as_str(),
        );

        if val.is_absolute() {
            return val;
        }

        command_dir_path.join(Path::new(self.0.as_str()))
    }
}

fn replace_smelt_root(input: &str, replacement: &str) -> PathBuf {
    let re = Regex::new(r"\$SMELT_ROOT|\$\{SMELT_ROOT\}").unwrap();
    let result = re.replace_all(input, replacement);
    PathBuf::from(result.into_owned())
}
