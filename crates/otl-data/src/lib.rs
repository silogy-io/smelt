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

tonic::include_proto!("otl_telemetry.data");

pub trait ToProtoMessage {
    type Message: prost::Message;

    fn as_proto(&self) -> Self::Message;
}

impl Event {
    pub fn new(et: Et) -> Self {
        Event {
            time: Some(std::time::SystemTime::now().into()),
            et: et.into(),
        }
    }
    pub fn finished_event(&self) -> bool {
        match self.et.as_ref().unwrap() {
            crate::event::Et::Done(_) => true,
            _ => false,
        }
    }

    pub fn done() -> Self {
        Self::new(Et::done())
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
}

impl CommandEvent {
    pub fn passed(&self) -> Option<bool> {
        self.command_variant.as_ref().unwrap().passed()
    }
}

impl ToProtoMessage for CommandEvent {
    type Message = Event;

    fn as_proto(&self) -> Self::Message {
        let time = Some(std::time::SystemTime::now().into());
        let et = Some(Et::Command(self.clone()));
        Event { time, et }
    }
}

use command_event::CommandVariant;
use prost_types::Timestamp;
impl CommandVariant {
    pub fn passed(&self) -> Option<bool> {
        match self {
            CommandVariant::Started(_) => None,
            CommandVariant::Cancelled(_) => None,
            CommandVariant::Finished(ref output) => Some(output.passed()),
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
        Self::Done(AllCommandsDone {})
    }
}
