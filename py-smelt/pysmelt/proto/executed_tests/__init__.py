# Generated by the protocol buffer compiler.  DO NOT EDIT!
# sources: executed_tests.proto
# plugin: python-betterproto
# This file has been @generated

from dataclasses import dataclass
from datetime import datetime
from typing import List

import betterproto


@dataclass(eq=False, repr=False)
class Digest(betterproto.Message):
    """Taken directly from the bazel reapi, for compat"""

    hash: str = betterproto.string_field(1)
    """
    The hash. In the case of SHA-256, it will always be a lowercase hex string
    exactly 64 characters long. in expectation, this should always be the
    sha256
    """

    size_bytes: int = betterproto.int64_field(2)
    """The size of the blob, in bytes."""


@dataclass(eq=False, repr=False)
class ArtifactPointer(betterproto.Message):
    path: str = betterproto.string_field(1, group="pointer")
    artifact_name: str = betterproto.string_field(3)


@dataclass(eq=False, repr=False)
class TestResult(betterproto.Message):
    """
    The serialized record of single test execution This is the api that we
    should build tools around -- if people provide tracked tests
    """

    test_name: str = betterproto.string_field(1)
    """name of the test"""

    outputs: "TestOutputs" = betterproto.message_field(2)


@dataclass(eq=False, repr=False)
class TestOutputs(betterproto.Message):
    artifacts: List["ArtifactPointer"] = betterproto.message_field(1)
    """Files that are expected from a test"""

    exit_code: int = betterproto.int32_field(2)
    """exit code of the test"""


@dataclass(eq=False, repr=False)
class Invocation(betterproto.Message):
    """
    Highest level invocation for a set of tests -- must contain one or more
    test results any time we run any test(s), an invocation object is created
    """

    invoke_id: str = betterproto.string_field(1)
    rundate: datetime = betterproto.message_field(2)
    user: str = betterproto.string_field(3)
    repo: str = betterproto.string_field(4)
    branch: str = betterproto.string_field(5)
    hostname: str = betterproto.string_field(6)
    smelt_root: str = betterproto.string_field(7)
    executed_tests: List["TestResult"] = betterproto.message_field(8)