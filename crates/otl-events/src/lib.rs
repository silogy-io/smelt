pub mod runtime_support;

pub use otl_data::{client_commands::ClientCommand, Event};

use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::{mpsc, oneshot},
};

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

pub type ClientCommandResp = Result<(), String>;

/// The "sink" used to communicate synchronously between a client and the otl runtime
/// this message will tell us if a "ClientCommand" is done
///
/// Wrapping this in our own type because it is likely we'll want to make the SyncSink to be
/// optional
pub struct SyncSink<T>(oneshot::Sender<T>);

impl<T> SyncSink<T> {
    fn send(self, message: T) -> Result<(), T> {
        self.0.send(message)
    }
}

pub struct EventSink<T>(mpsc::Sender<T>);

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
