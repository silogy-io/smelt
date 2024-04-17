use std::{path::PathBuf, time::Instant};
pub mod runtime_support;
use serde::{Deserialize, Serialize};

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct Event {
    pub time: Instant,
    pub et: OtlEvent,
}

impl Event {
    pub fn finished_event(&self) -> bool {
        match self.et {
            OtlEvent::AllCommandsDone => true,
            _ => false,
        }
    }

    pub fn new(et: OtlEvent) -> Self {
        Self {
            time: std::time::Instant::now(),
            et,
        }
    }

    pub fn new_command_event(command_ref: String, et: CommandVariant) -> Self {
        let time = std::time::Instant::now();
        let et = OtlEvent::Command(CommandEvent {
            command_ref,
            inner: et,
        });
        Self { time, et }
    }
}

#[derive(Clone, Debug)]
pub enum OtlEvent {
    AllCommandsDone,
    Command(CommandEvent),
}

#[derive(Clone, Debug)]
pub struct CommandEvent {
    pub command_ref: String,
    inner: CommandVariant,
}

impl CommandEvent {
    pub fn passed(&self) -> Option<bool> {
        match self.inner {
            CommandVariant::CommandStarted => None,
            CommandVariant::CommandCancelled => None,
            CommandVariant::CommandFinished(ref output) => Some(output.passed()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum CommandVariant {
    CommandStarted,
    CommandCancelled,
    CommandFinished(CommandOutput),
}

#[derive(Clone, Dupe, PartialEq, Eq, Hash, Display, Debug, Allocative, Serialize, Deserialize)]
pub struct CommandOutput {
    pub status_code: i32,
}

impl CommandOutput {
    pub fn passed(&self) -> bool {
        self.status_code == 0
    }

    const fn asfile() -> &'static str {
        "command.status"
    }
    pub async fn to_file(&self, _base_path: &PathBuf) -> Result<(), std::io::Error> {
        let mut command_out = File::create(CommandOutput::asfile()).await?;

        command_out
            .write(serde_json::to_vec(self)?.as_slice())
            .await?;
        Ok(())
    }
}
