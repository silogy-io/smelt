from typing import TypedDict
from abc import ABC


class SimulatorProvider(TypedDict):
    simulator: str


class SimulatorTarget(ABC):
    def to_buck2_target(self) -> str:
        ...

    def outputs(self) -> SimulatorProvider:
        ...

    @classmethod
    def generate_target(self):
        ...
