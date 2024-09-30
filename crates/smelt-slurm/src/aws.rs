use std::path::PathBuf;

use aws_config::BehaviorVersion;
use aws_sdk_s3 as s3;

use aws_credential_types::Credentials;

use aws_smithy_types::byte_stream::{ByteStream, Length};
use s3::{
    operation::create_multipart_upload::CreateMultipartUploadOutput,
    types::{CompletedMultipartUpload, CompletedPart},
};

const AWS_REGION: &str = "us-west-1";

// these constants are arbitrarily chosen, tbh
// 1mb chunk size
const CHUNK_SIZE: u64 = 1024 * 1024;
// 10gb max artifact size
const MAX_CHUNKS: u64 = 10000;

/// All of the required data to create an AWS Client and upload artifacts to an s3 bucket
pub struct AwsCreds {
    pub key_id: String,
    pub key: String,
    // the name of the bucket
    pub bucket: String,
    // the key base path for when we're uploading each object
    // useful for "namespacing" across executions
    pub key_base_path: String,
}

pub async fn create_s3_client(cred: &AwsCreds) -> Result<s3::Client, s3::Error> {
    let creds = Credentials::new(
        cred.key_id.as_str(),
        cred.key.as_str(),
        None,
        None,
        "smelt-worker",
    );

    // Look man, i dont know why credentials_provider has a static trait bound
    // whatever works
    // but
    // we are hardcoding support for only us-west-1 because of it
    //
    // amz, you make me sin
    //
    // and for what?

    let shared_config = aws_config::defaults(BehaviorVersion::v2024_03_28())
        .region(AWS_REGION)
        .credentials_provider(creds)
        .load()
        .await;
    let client = s3::Client::new(&shared_config);
    Ok(client)
}

pub async fn upload_file(
    command_name: &str,
    client: &s3::Client,
    creds: &AwsCreds,
    file_path: PathBuf,
) -> anyhow::Result<String> {
    let key = format!(
        "{}/{}/artifacts/{}",
        creds.key_base_path,
        command_name,
        file_path.file_name().unwrap().to_string_lossy()
    );
    let multipart_upload_res: CreateMultipartUploadOutput = client
        .create_multipart_upload()
        .bucket(&creds.bucket)
        .key(&key)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create multipart obj with err {e:?}"))?;

    let upload_id = multipart_upload_res.upload_id().ok_or(anyhow::anyhow!(
        "Missing upload_id after CreateMultipartUpload",
    ))?;

    let file_size = tokio::fs::metadata(&file_path)
        .await
        .expect("it exists I swear")
        .len();

    let mut chunk_count = (file_size / CHUNK_SIZE) + 1;
    let mut size_of_last_chunk = file_size % CHUNK_SIZE;
    if size_of_last_chunk == 0 {
        size_of_last_chunk = CHUNK_SIZE;
        chunk_count -= 1;
    }

    if file_size == 0 {
        anyhow::bail!("Bad file size.")
    }
    if chunk_count > MAX_CHUNKS {
        anyhow::bail!("Too many chunks! Try increasing your chunk size.")
    }

    let mut upload_parts: Vec<aws_sdk_s3::types::CompletedPart> = Vec::new();

    for chunk_index in 0..chunk_count {
        let this_chunk = if chunk_count - 1 == chunk_index {
            size_of_last_chunk
        } else {
            CHUNK_SIZE
        };
        let stream = ByteStream::read_from()
            .path(file_path.as_path())
            .offset(chunk_index * CHUNK_SIZE)
            .length(Length::Exact(this_chunk))
            .build()
            .await
            .unwrap();

        // Chunk index needs to start at 0, but part numbers start at 1.
        let part_number = (chunk_index as i32) + 1;
        let upload_part_res = client
            .upload_part()
            .key(&key)
            .bucket(&creds.bucket)
            .upload_id(upload_id)
            .body(stream)
            .part_number(part_number)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Failed to upload chunk {part_number}"))?;

        upload_parts.push(
            CompletedPart::builder()
                .e_tag(upload_part_res.e_tag.unwrap_or_default())
                .part_number(part_number)
                .build(),
        );
    }

    // upload_parts: Vec<aws_sdk_s3::types::CompletedPart>
    let completed_multipart_upload: CompletedMultipartUpload = CompletedMultipartUpload::builder()
        .set_parts(Some(upload_parts))
        .build();

    let _complete_multipart_upload_res = client
        .complete_multipart_upload()
        .bucket(&creds.bucket)
        .key(&key)
        .multipart_upload(completed_multipart_upload)
        .upload_id(upload_id)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to complete the upload with err {e:?}"))?;

    Ok(key)
}
