from dataclasses import dataclass
from typing import List, Optional, Dict


@dataclass
class CasHash:
    """
    Taken from bazel reapi -- preserved

    """

    digest: str
    size_bytes: int


@dataclass
class ArtifactCas:
    """
    Artifacts are assumed to be files

    """

    hash: CasHash
    artifact_name: str


@dataclass
class TestMetaData:
    """
    User name of who owns the test
    """

    test_owner: str
    """
    name of the design under test -- useful for tracking across different
    testbenches
    """
    design_under_test: str

    """
    Any extra data end-users might want to tag onto a test, for test
    organization 

    """
    extras: Dict[str, str]


@dataclass
class TestResult:
    """
    The serialized record of single test execution
    """

    invoke_id: str
    """
    same key from Invocation 
    """
    test_name: str
    """
    name of the test 
    """
    test_def_id: int
    """
    test definition id -- unique id to track the test across different
    Invocations 

    useful for test history
    """

    """
    maps name of a file to a hash of the file contents

    To start off, we should only support files here 

    """
    artifacts: List[ArtifactCas]

    """ 
    some useful metadata for each test execution 
    """
    metadata: Optional[TestMetaData]

    """
    cli reproduction -- we might want to track this a level above
    """
    repro_command: str


@dataclass
class Invocation:
    """
    Highest level invocation for a test
    """

    invoke_id: str  # uuid
    rundate: str  # date
    user: str
    repo: str
    branch: Optional[str]
    hostname: str
    os: str
    executed_tests: List[TestResult]
