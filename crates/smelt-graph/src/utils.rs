use dice::{DiceData, UserComputationData};
use smelt_data::{invoke_event::InvokeVariant, Event, ExecutionStart};
use smelt_events::{
    new_invoke_event,
    runtime_support::{GetSmeltRoot, GetTraceId},
};
use tokio::process::Command;
use whoami::fallible;

//TODO: for gha, gitlab, etc, they have bespoke ways of communicating git info
//      we need to support that
async fn get_git_info() -> (String, String, String) {
    let hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .await
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string();

    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .await
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string();

    let repo = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .await
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string();

    (hash, branch, repo)
}

async fn exec_info(global_data: &DiceData) -> ExecutionStart {
    let hostname = fallible::hostname().unwrap_or("unknown_host".to_string());
    let username = fallible::username().unwrap_or("unkown_user".to_string());

    let smelt_root = global_data.get_smelt_root().to_string_lossy().to_string();
    let (git_hash, git_branch, git_repo) = get_git_info().await;
    //TODO fill this in
    ExecutionStart {
        hostname,
        username,
        smelt_root,
        git_hash,
        git_branch,
        git_repo,
    }
}

pub async fn invoke_start_message(
    user_data: &UserComputationData,
    global_data: &DiceData,
) -> Event {
    let invoke_variant = InvokeVariant::Start(exec_info(global_data).await);
    let trace_id = user_data.get_trace_id();
    new_invoke_event(trace_id, invoke_variant)
}
