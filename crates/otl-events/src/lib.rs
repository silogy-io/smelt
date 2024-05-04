pub mod runtime_support;

pub use otl_data::Event;

use tokio::{fs::File, io::AsyncWriteExt};

pub use helpers::*;
mod helpers {
    use std::path::Path;

    use super::*;
    use otl_data::{command_event::CommandVariant, invoke_event::InvokeVariant, InvokeEvent};
    use otl_data::{event::Et, CommandOutput};
    use otl_data::{CommandEvent, Event};

    pub fn new_command_event(
        command_ref: String,
        inner: CommandVariant,
        trace_id: String,
    ) -> Event {
        let time = std::time::SystemTime::now();
        let et = Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(inner),
        });
        Event {
            trace_id,
            time: Some(time.into()),
            et: Some(et),
        }
    }

    pub fn new_invoke_event(trace_id: String, invoke_event: InvokeVariant) -> Event {
        let time = std::time::SystemTime::now();
        let et = Et::Invoke(InvokeEvent {
            invoke_variant: Some(invoke_event),
        });
        Event {
            trace_id,
            time: Some(time.into()),
            et: Some(et),
        }
    }

    const fn output_asfile() -> &'static str {
        "command.status"
    }
    pub async fn to_file(out: &CommandOutput, _base_path: &Path) -> Result<(), std::io::Error> {
        let mut command_out = File::create(output_asfile()).await?;

        command_out
            .write_all(serde_json::to_vec(out)?.as_slice())
            .await?;
        Ok(())
    }
}
