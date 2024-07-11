tonic::include_proto!("executed_tests");
use allocative::Allocative;
use std::path::PathBuf;
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

#[derive(Allocative, Clone)]
pub enum ExecutedTestResult {
    Success(TestResult),
    MissingFiles {
        /// this contains the test result, with all the files that exist
        test_result: TestResult,
        /// artifacts that are missing -- will always point to the filesystem
        missing_artifacts: Vec<ArtifactPointer>,
    },
    Skipped,
}

impl ExecutedTestResult {
    pub fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped)
    }

    pub fn test_name(&self) -> String {
        self.clone().to_test_result().test_name
    }

    pub fn to_test_result(self) -> TestResult {
        match self {
            Self::Success(val) => val,
            Self::MissingFiles { test_result, .. } => test_result,
            Self::Skipped => TestResult::default(),
        }
    }
    pub fn get_retcode(&self) -> i32 {
        match self {
            Self::Success(val) => val.outputs.as_ref().map(|val| val.exit_code).unwrap(),
            Self::MissingFiles { test_result, .. } => test_result
                .outputs
                .as_ref()
                .map(|val| val.exit_code)
                .unwrap(),
            Self::Skipped => {
                tracing::error!(
                    "Getting the retcode for a skipped testresult -- this is unexpected"
                );
                -1
            }
        }
    }
    pub fn failed(&self) -> bool {
        match self {
            Self::Success(val) => val.outputs.as_ref().map(|val| val.exit_code).unwrap() != 0,
            Self::MissingFiles { test_result, .. } => {
                test_result
                    .outputs
                    .as_ref()
                    .map(|val| val.exit_code)
                    .unwrap()
                    != 0
            }
            Self::Skipped => false,
        }
    }
}

impl TestOutputs {
    pub fn passed(&self) -> bool {
        self.exit_code == 0
    }
}
