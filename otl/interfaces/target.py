from dataclasses import dataclass
from abc import ABC
from enum import Enum
from typing import List, Dict
from otl.interfaces.runtime import RuntimeRequirements
from otl.interfaces.paths import OtlPath


class OtlTargetType(Enum):
    Test = "test"
    Stimulus = "stimulus"
    Build = "build"


TargetRef = str


@dataclass
class Target(ABC):
    name: str

    def get_outputs(self) -> Dict[str, OtlPath]:
        return {}

    def gen_script(self) -> List[str]:
        ...

    @staticmethod
    def rule_type() -> OtlTargetType:
        return OtlTargetType.Test

    def runtime_env_vars(self) -> Dict[str, str]:
        return {}

    def runtime_requirements(self) -> RuntimeRequirements:
        return RuntimeRequirements.default()

    def dependencies(self) -> List[TargetRef]:
        return []
