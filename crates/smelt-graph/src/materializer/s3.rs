use super::Materializer;
use aws_config::BehaviorVersion;
use aws_sdk_s3 as s3;

use aws_credential_types::{provider::ProvideCredentials, Credentials};

pub async fn create_s3_client(
    key_id: &str,
    key: &str,
    region: &'static str,
) -> Result<s3::Client, s3::Error> {
    let creds = Credentials::new(key_id, key, None, None, "rascal");
    let shared_config = aws_config::defaults(BehaviorVersion::v2024_03_28())
        .region(region)
        .credentials_provider(creds)
        .load()
        .await;
    let client = s3::Client::new(&shared_config);
    Ok(client)
}

pub struct S3Materializer(s3::Client);

impl Materializer for S3Materializer {
    fn preserve(
        &self,
        command: smelt_core::Command,
        test_result: smelt_data::executed_tests::TestResult,
    ) -> anyhow::Result<()> {
        todo!();
    }
}
