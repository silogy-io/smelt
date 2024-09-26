use std::path::PathBuf;

use argh::FromArgs;
use smelt_slurm::{execute_command, AwsCreds};

#[derive(FromArgs, Debug, Clone)]
/// Worker args
struct WorkerArgs {
    #[argh(option)]
    /// path to the command working directory
    command_path: PathBuf,

    #[argh(option)]
    /// path to the bash script to execute
    command_name: String,

    #[argh(option)]
    /// hostname of the smelt server to capture events
    host: String,

    /// the trace id of the actual smelt execution going on
    #[argh(option)]
    trace_id: String,

    /// aws key id -- used to create s3 client
    #[argh(option)]
    aws_key_id: Option<String>,

    /// aws key -- used to create s3 client
    #[argh(option)]
    aws_key: Option<String>,

    /// aws bucket -- bucket to upload to
    #[argh(option)]
    aws_bucket: Option<String>,

    /// base path to be used for the s3key -- we use this as a "root" file path to start from for
    /// uploading to a particular object for a key/bucket combo
    ///
    /// if not present, base path with be trace-id
    #[argh(option)]
    s3_key_base_path: Option<String>,
}

fn main() {
    let args: WorkerArgs = argh::from_env();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    // keep them around to printout if we fail
    let dbg_args = args.clone();

    let WorkerArgs {
        command_name,
        command_path,
        host,
        trace_id,
        aws_key,
        aws_key_id,
        aws_bucket,
        s3_key_base_path,
    } = args;

    let key_base_path = s3_key_base_path.unwrap_or(trace_id.clone());
    let creds = match (aws_key, aws_key_id, aws_bucket) {
        (Some(key), Some(key_id), Some(bucket)) => Some(AwsCreds {
            key,
            key_id,
            bucket,
            key_base_path,
        }),
        (None, None, None) => None,
        _ => panic!("Did not provide all of the execpted aws credentials!"),
    };

    rt.block_on(execute_command(
        command_name.as_str(),
        command_path,
        trace_id,
        host,
        creds,
    ))
    .unwrap_or_else(|_| panic!("There was a failure executing the command!\n\ncli args are {dbg_args:?}"));
}
