use event::Et;

mod serialize_timestamp {
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serialize;
    use serde::Serializer;

    pub fn serialize<S>(
        value: &Option<::prost_types::Timestamp>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let d = value.as_ref().map(|v| (v.seconds, v.nanos));
        d.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<::prost_types::Timestamp>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = Option::<(i64, i32)>::deserialize(deserializer)?;
        let d = d.map(|(seconds, nanos)| ::prost_types::Timestamp { seconds, nanos });
        Ok(d)
    }
}

mod serialize_bytes {
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serialize;
    use serde::Serializer;

    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let d = hex::encode(value);
        d.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = String::deserialize(deserializer)?;
        let d = hex::decode(d).map_err(serde::de::Error::custom)?;
        Ok(d)
    }
}

pub mod client_commands;

tonic::include_proto!("otl_telemetry.data");

impl Event {
    pub fn new(et: Et, trace_id: String) -> Self {
        Event {
            trace_id,
            time: Some(std::time::SystemTime::now().into()),
            et: et.into(),
        }
    }

    pub fn command_started(command_ref: String, trace_id: String) -> Self {
        let et = event::Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(CommandVariant::Started(CommandStarted {})),
        });
        Self::new(et, trace_id)
    }

    pub fn command_finished(
        command_ref: String,
        trace_id: String,
        comm_out: CommandOutput,
    ) -> Self {
        let et = event::Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(CommandVariant::Finished(CommandFinished {
                out: Some(comm_out),
            })),
        });
        Self::new(et, trace_id)
    }

    pub fn command_stdout(command_ref: String, trace_id: String, stdout: String) -> Self {
        let et = event::Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(CommandVariant::Stdout(CommandStdout { output: stdout })),
        });
        Self::new(et, trace_id)
    }

    pub fn finished_event(&self) -> bool {
        matches!(
            self.et.as_ref().unwrap(),
            crate::event::Et::Invoke(InvokeEvent {
                invoke_variant: Some(invoke_event::InvokeVariant::Done(_)),
            })
        )
    }

    pub fn done(trace_id: String) -> Self {
        Self::new(Et::done(), trace_id)
    }

    pub fn command_output(&self) -> Option<CommandOutput> {
        self.et
            .as_ref()
            .and_then(|val| match val {
                Et::Command(comm) => comm.command_variant.clone().and_then(|inner| match inner {
                    CommandVariant::Finished(CommandFinished { out }) => Some(out),
                    _ => None,
                }),

                _ => None,
            })
            .flatten()
    }

    pub fn client_error(payload: String) -> Event {
        let trace_id = "CLIENT_ERROR".to_string();
        let et = Et::Error(OtlError {
            sig: OtlErrorType::ClientError.into(),
            error_payload: payload,
        });
        Event::new(et, trace_id)
    }
}

impl CommandEvent {
    pub fn passed(&self) -> Option<bool> {
        self.command_variant.as_ref().unwrap().passed()
    }
}

use command_event::CommandVariant;

impl CommandVariant {
    pub fn passed(&self) -> Option<bool> {
        match self {
            CommandVariant::Finished(ref output) => Some(output.passed()),
            _ => None,
        }
    }
}

impl CommandFinished {
    pub fn passed(&self) -> bool {
        self.out.as_ref().unwrap().passed()
    }
}

impl CommandOutput {
    pub fn passed(&self) -> bool {
        self.status_code == 0
    }
}

impl Et {
    pub fn done() -> Self {
        crate::event::Et::Invoke(InvokeEvent {
            invoke_variant: Some(invoke_event::InvokeVariant::Done(AllCommandsDone {})),
        })
    }
}
