use std::{
    path::PathBuf,
    time::{Instant, SystemTime},
};
pub mod runtime_support;
use serde::{Deserialize, Serialize};

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
pub use otl_data::Event;

use tokio::{fs::File, io::AsyncWriteExt};

pub use helpers::*;
mod helpers {
    use super::*;
    use otl_data::{command_event::CommandVariant, CommandFinished};
    use otl_data::{event::Et, CommandOutput};
    use otl_data::{CommandEvent, Event};

    pub fn new_command_event(command_ref: String, inner: CommandVariant) -> Event {
        let time = std::time::SystemTime::now();
        let et = Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(inner),
        });
        Event {
            time: Some(time.into()),
            et: Some(et),
        }
    }

    const fn output_asfile() -> &'static str {
        "command.status"
    }
    pub async fn to_file(out: &CommandOutput, _base_path: &PathBuf) -> Result<(), std::io::Error> {
        let mut command_out = File::create(output_asfile()).await?;

        command_out
            .write(serde_json::to_vec(out)?.as_slice())
            .await?;
        Ok(())
    }
}
