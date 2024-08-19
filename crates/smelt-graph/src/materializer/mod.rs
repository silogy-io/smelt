use smelt_core::Command;
use smelt_data::executed_tests::TestResult;

/// The materializer is responsible for post processing artifacts of each command
pub trait Materializer {
    fn preserve(&self, command: Command, test_result: TestResult) -> anyhow::Result<()>;
}

/// Pass through materializer -- does nothing
pub struct PTMat;

impl Materializer for PTMat {
    fn preserve(&self, command: Command, test_result: TestResult) -> anyhow::Result<()> {
        Ok(())
    }
}

mod local;
#[cfg(feature = "s3")]
mod s3;
