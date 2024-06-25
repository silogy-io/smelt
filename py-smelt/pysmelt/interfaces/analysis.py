from typing import List
from pysmelt.proto.executed_tests import Invocation, TestResult
from pysmelt.interfaces.paths import SmeltPath


def most_recent_invoke_path() -> SmeltPath:
    """
    By default, we push the most recent invocation to this path

    """
    return SmeltPath("smelt-out/invocation.bin")


def get_previous_invocation() -> Invocation:
    invbytes = open(most_recent_invoke_path().to_abs_path(), "rb").read()
    return Invocation.FromString(invbytes)


def most_recent_tests_run() -> List[TestResult]:
    return get_previous_invocation().executed_tests
