from typing import Dict
from abc import ABC


class TestTarget(ABC):
    def to_buck2_rule(self) -> str:
        ...

    def outputs(self) -> Dict[str, str]:
        ...

    @classmethod
    def generate_target(self):
        ...
