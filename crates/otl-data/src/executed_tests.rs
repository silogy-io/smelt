use allocative::Allocative;
use std::path::PathBuf;

tonic::include_proto!("executed_tests");

impl ArtifactPointer {
    pub fn file_artifact(artifact_name: String, path: PathBuf) -> Self {
        let abs_path = path.to_string_lossy().to_string();
        let pointer = Some(artifact_pointer::Pointer::Path(abs_path));
        Self {
            artifact_name,
            pointer,
        }
    }
}

#[derive(Allocative)]
pub enum ExecutedTestResult {
    Success(TestResult),
    MissingFiles {
        /// this contains the test result, with all the files that exist
        test_result: TestResult,
        /// artifacts that are missing -- will always point to the filesystem
        missing_artifacts: Vec<ArtifactPointer>,
    },
}

impl ExecutedTestResult {
    pub fn to_test_result(self) -> TestResult {
        match self {
            Self::Success(val) => val,
            Self::MissingFiles { test_result, .. } => test_result,
        }
    }
    pub fn get_retcode(&self) -> i32 {
        match self {
            Self::Success(val) => val.exit_code,
            Self::MissingFiles { test_result, .. } => test_result.exit_code,
        }
    }
}
