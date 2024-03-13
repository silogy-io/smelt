from typing import TypedDict
from abc import ABC


class TestTargetProvider(TypedDict):
    results_dir: str


class TestTarget(ABC):
    def to_buck2_target(self) -> str:
        ...

    def outputs(self) -> TestTargetProvider:
        ...

    @classmethod
    def execute_test(self):
        ...
