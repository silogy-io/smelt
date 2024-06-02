use smelt_data::{invoke_event::InvokeVariant, Event, ExecutionStart};
use smelt_events::new_invoke_event;
use whoami::fallible;

fn exec_info() -> ExecutionStart {
    let hostname = fallible::hostname().unwrap_or("unknown_host".to_string());
    let username = fallible::username().unwrap_or("unkown_user".to_string());
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
