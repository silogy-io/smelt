from abc import ABC
from typing import Dict


class SimulatorTarget(ABC):
    def to_buck2_rule(self) -> str:
        ...

    def outputs(self) -> Dict[str, str]:
        ...

    @classmethod
    def generate_target(self):
        ...
