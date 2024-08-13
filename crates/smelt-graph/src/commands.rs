use allocative::Allocative;
use dupe::Dupe;

pub use smelt_core::{Command, CommandDependency, Runtime, TargetType};

use serde::{Deserialize, Serialize};

use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use smelt_core::SmeltErr;

use smelt_core::CommandDefPath;
