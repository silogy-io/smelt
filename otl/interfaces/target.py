from abc import ABC
from enum import Enum
from typing import List, Dict
from otl.interfaces.action_status import RuntimeRequirements
from otl.interfaces.paths import OtlPath


class OtlTargetType(Enum):
    Test = "test"
    Stimulus = "stimulus"
    Build = "build"


class Target(ABC):
    name: str

    def get_outputs(self) -> Dict[str, OtlPath]:
        ...

    def gen_script(self) -> List[str]:
        ...

    def runtime_env_vars(self) -> Dict[str, str]:
        return {}

    def runtime_requirements(self) -> RuntimeRequirements:
        return RuntimeRequirements.default()

    def target_type(self) -> OtlTargetType:
        return OtlTargetType.Test
