use std::{
    collections::{HashMap},
    path::PathBuf,
};





type CommandHandle = String;
//#[async_trait]
//impl Subscriber for Tracker {}
//
//

enum OutputLookinThang {
    InMemory(String),
    OnDisk(PathBuf),
}

enum ExecCommandState {
    Running,
    Cancelled,
    Completed { status_code: i32 },
}

struct ExecCommand {
    status: ExecCommandState,
    stdout: OutputLookinThang,
    stderr: OutputLookinThang,
}

struct InvokerMetaData {}

pub struct InvocationTracker {
    invoker: InvokerMetaData,
    command_map: HashMap<CommandHandle, ExecCommandState>,
}
