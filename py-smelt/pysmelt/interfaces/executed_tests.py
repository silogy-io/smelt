# Generated by the protocol buffer compiler.  DO NOT EDIT!
# sources: executed_tests.proto
# plugin: python-betterproto
from dataclasses import dataclass
from typing import Dict, List

import betterproto


@dataclass
class Digest(betterproto.Message):
    """Taken directly from the bazel reapi, for compat"""

    # The hash. In the case of SHA-256, it will always be a lowercase hex string
    # exactly 64 characters long. in expectation, this should always be the
    # sha256
    hash: str = betterproto.string_field(1)
    # The size of the blob, in bytes.
    size_bytes: int = betterproto.int64_field(2)


@dataclass
class ArtifactPointer(betterproto.Message):
    cas_hash: "Digest" = betterproto.message_field(1, group="pointer")
    path: str = betterproto.string_field(2, group="pointer")
    artifact_name: str = betterproto.string_field(3)


@dataclass
class TestMetaData(betterproto.Message):
    # User name of who owns the test
    test_owner: str = betterproto.string_field(1)
    # name of the design under test -- useful for tracking across different
    # testbenches
    design_under_test: str = betterproto.string_field(2)
    # Any extra data end-users might want to tag onto a test, for test
    # organization
    extras: Dict[str, str] = betterproto.map_field(
        3, betterproto.TYPE_STRING, betterproto.TYPE_STRING
    )


@dataclass
class TestResult(betterproto.Message):
    """
    The serialized record of single test execution A backend that stores this
    data should normalize the data in this object into a "TestDef" and a
    "TestInstantiation". I don't think it's complexity that should be exposed
    to the frontend
    """

    # same key from Invocation
    invoke_id: str = betterproto.string_field(1)
    # name of the test
    test_name: str = betterproto.string_field(2)
    # test definition id -- unique id to track the test across different
    # Invocations, useful for test history
    test_def_id: str = betterproto.string_field(3)
    # maps name of a file to a hash of the file contents To start off, we should
    # only support files here
    artifacts: List["ArtifactPointer"] = betterproto.message_field(4)
    # useful metadata for each test execution
    metadata: "TestMetaData" = betterproto.message_field(5)
    # cli reproduction -- we might want to track this a level above
    repro_command: str = betterproto.string_field(6)


@dataclass
class Invocation(betterproto.Message):
    # Highest level invocation for a set of tests -- must contain one or more
    # test results any time we run any test(s), an invocation object is created
    invoke_id: str = betterproto.string_field(1)
    rundate: str = betterproto.string_field(2)
    user: str = betterproto.string_field(3)
    repo: str = betterproto.string_field(4)
    branch: str = betterproto.string_field(5)
    hostname: str = betterproto.string_field(6)
    executed_tests: List["TestResult"] = betterproto.message_field(8)