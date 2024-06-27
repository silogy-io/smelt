from typing import List, Optional
from pysmelt.proto.executed_tests import Invocation, TestResult
from pysmelt.interfaces.paths import SmeltPath
from dataclasses import dataclass


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


def read_log(test_result: TestResult) -> str:
    log_name = "smelt_log"
    log = next(
        (
            artifact
            for artifact in test_result.outputs.artifacts
            if artifact.artifact_name == log_name
        )
    )
    return open(log.path, "r").read()


@dataclass(frozen=True)
class IQL:
    """
    This is the "Invocation Query Layer" -- a helper class to get data out from Invocations

    """

    inv: Invocation

    @classmethod
    def from_previous(cls):
        return cls(inv=get_previous_invocation())

    def get_test(self, test_name: str) -> Optional[TestResult]:
        return next(
            (test for test in self.inv.executed_tests if test.test_name == test_name),
            None,
        )

    def get_tests_from_testgroup(
        self, test_group_name: str
    ) -> Optional[List[TestResult]]:
        test_group = self.get_test(test_group_name)
        if test_group:
            log_content = read_log(test_group)
            test_names = log_content.split("\n")
            return [
                test
                for test in (self.get_test(tn) for tn in test_names)
                if test is not None
            ]
        else:
            return None
