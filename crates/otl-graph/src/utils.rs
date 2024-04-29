
use otl_data::{invoke_event::InvokeVariant, Event, ExecutionStart};
use otl_events::new_invoke_event;

fn exec_info() -> ExecutionStart {
    let hostname = whoami::hostname();
    let username = whoami::username();
    let path = std::env::current_dir()
        .map(|buf| buf.to_string_lossy().to_string())
        .unwrap_or("unknown_path".to_string())
        .to_string();
    ExecutionStart {
        hostname,
        username,
        path,
    }
}

pub fn invoke_start_message(trace_id: String) -> Event {
    let invoke_variant = InvokeVariant::Start(exec_info());
    new_invoke_event(trace_id, invoke_variant)
}
