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

//mod serialize_bytes {
//    use serde::Deserialize;
//    use serde::Deserializer;
//    use serde::Serialize;
//    use serde::Serializer;
//
//    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
//    where
//        S: Serializer,
//    {
//        let d = hex::encode(value);
//        d.serialize(serializer)
//    }
//
//    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
//    where
//        D: Deserializer<'de>,
//    {
//        let d = String::deserialize(deserializer)?;
//        let d = hex::decode(d).map_err(serde::de::Error::custom)?;
//        Ok(d)
//    }
//}

pub mod client_commands;
pub mod executed_tests;

pub mod smelt_telemetry {

    tonic::include_proto!("smelt_telemetry");
}
use executed_tests::TestResult;
pub use smelt_telemetry::*;

impl Event {
    pub fn new(et: Et, trace_id: String) -> Self {
        Event {
            trace_id,
            time: Some(std::time::SystemTime::now().into()),
            et: et.into(),
        }
    }

    pub fn from_command_variant(
        command_ref: String,
        trace_id: String,
        variant: CommandVariant,
    ) -> Self {
        let et = event::Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(variant),
        });
        Self::new(et, trace_id)
    }
    pub fn command_started(command_ref: String, trace_id: String) -> Self {
        let et = event::Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(CommandVariant::Started(CommandStarted {})),
        });
        Self::new(et, trace_id)
    }

    pub fn command_finished(test: TestResult, command_type: String, trace_id: String) -> Self {
        let command_ref = test.test_name;
        let to = test.outputs.unwrap();
        let et = event::Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(CommandVariant::Finished(CommandFinished {
                outputs: Some(to),
                command_type,
            })),
        });
        Self::new(et, trace_id)
    }

    pub fn command_skipped(command_ref: String, trace_id: String) -> Self {
        let et = event::Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(CommandVariant::Skipped(CommandSkipped {})),
        });
        Self::new(et, trace_id)
    }

    pub fn command_scheduled(command_ref: String, trace_id: String) -> Self {
        let et = event::Et::Command(CommandEvent {
            command_ref,
            command_variant: Some(CommandVariant::Scheduled(CommandScheduled {})),
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

    pub fn as_result(&self) -> Option<TestResult> {
        match self {
            Event {
                et:
                    Some(Et::Command(CommandEvent {
                        command_variant:
                            Some(CommandVariant::Finished(CommandFinished { outputs, .. })),
                        command_ref,
                    })),
                ..
            } => Some(TestResult {
                test_name: command_ref.clone(),
                outputs: outputs.clone(),
            }),

            _ => None,
        }
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

    pub fn set_graph() -> Self {
        Self::new(Et::set_graph(), "".to_string())
    }

    //    pub fn command_output(&self) -> Option<CommandOutput> {
    //        self.et
    //            .as_ref()
    //            .and_then(|val| match val {
    //                Et::Command(comm) => comm.command_variant.clone().and_then(|inner| match inner {
    //                    CommandVariant::Finished(CommandFinished { out }) => Some(out),
    //                    _ => None,
    //                }),
    //
    //                _ => None,
    //            })
    //            .flatten()
    //    }

    pub fn client_error(payload: String) -> Event {
        let trace_id = "CLIENT_ERROR".to_string();
        let et = Et::Error(SmeltError {
            sig: SmeltErrorType::ClientError.into(),
            error_payload: payload,
        });
        Event::new(et, trace_id)
    }
    pub fn runtime_error(payload: String, trace_id: String) -> Event {
        let et = Et::Error(SmeltError {
            sig: SmeltErrorType::InternalError.into(),
            error_payload: payload,
        });
        Event::new(et, trace_id)
    }

    pub fn runtime_warn(payload: String, trace_id: String) -> Event {
        let et = Et::Error(SmeltError {
            sig: SmeltErrorType::InternalWarn.into(),
            error_payload: payload,
        });
        Event::new(et, trace_id)
    }

    pub fn graph_validate_error(payload: String) -> Event {
        Self::runtime_error(payload, "VALIDATE_ERROR".to_string())
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
        self.outputs.as_ref().unwrap().passed()
    }
}

impl Et {
    pub fn done() -> Self {
        crate::event::Et::Invoke(InvokeEvent {
            invoke_variant: Some(invoke_event::InvokeVariant::Done(AllCommandsDone {})),
        })
    }

    pub fn set_graph() -> Self {
        crate::event::Et::Invoke(InvokeEvent {
            invoke_variant: Some(invoke_event::InvokeVariant::Set(SetGraph {})),
        })
    }
}
