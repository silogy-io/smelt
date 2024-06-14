pub mod runtime_support;

use smelt_data::client_commands::{ClientResp};
pub use smelt_data::{client_commands::ClientCommand, Event};

use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::{mpsc, oneshot},
};

pub use helpers::*;
mod helpers {
    use std::path::Path;

    use super::*;
    use smelt_data::{command_event::CommandVariant, invoke_event::InvokeVariant, InvokeEvent};
    use smelt_data::{event::Et, executed_tests::TestOutputs};
    use smelt_data::{CommandEvent, Event};

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
    pub async fn to_file(out: &TestOutputs, _base_path: &Path) -> Result<(), std::io::Error> {
        let mut command_out = File::create(output_asfile()).await?;

        command_out
            .write_all(serde_json::to_vec(out)?.as_slice())
            .await?;
        Ok(())
    }
}

pub type ClientCommandResp = Result<ClientResp, String>;

pub struct ClientCommandBundle {
    pub message: ClientCommand,
    pub oneshot_confirmer: oneshot::Sender<ClientCommandResp>,
    pub event_streamer: mpsc::Sender<Event>,
}

pub struct EventStreams {
    pub sync_chan: oneshot::Receiver<ClientCommandResp>,
    pub event_stream: mpsc::Receiver<Event>,
}

impl ClientCommandBundle {
    pub fn from_message(message: ClientCommand) -> (Self, EventStreams) {
        let (oneshot_confirmer, sync_chan) = tokio::sync::oneshot::channel();
        let (event_streamer, event_stream) = mpsc::channel(100);
        (
            Self {
                message,
                oneshot_confirmer,
                event_streamer,
            },
            EventStreams {
                sync_chan,
                event_stream,
            },
        )
    }
}
