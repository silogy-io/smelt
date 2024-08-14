use std::path::PathBuf;

use argh::FromArgs;
use smelt_slurm::execute_command;

#[derive(FromArgs, Debug)]
/// Worker args
struct WorkerArgs {
    #[argh(option)]
    /// path to the bash script to execute
    command_path: PathBuf,

    #[argh(option)]
    /// path to the bash script to execute
    command_name: String,

    #[argh(option)]
    /// hostname of the smelt server to capture events
    host: String,

    /// port of the server
    #[argh(option)]
    trace_id: String,
}

fn main() {
    let args: WorkerArgs = argh::from_env();
    println!("Hey boo");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let WorkerArgs {
        command_name,
        command_path,
        host,
        trace_id,
    } = args;

    rt.block_on(execute_command(
        command_name.as_str(),
        command_path,
        trace_id,
        host,
    ))
    .expect("There was a failure executing the command!");
    println!("Done boo");
}
