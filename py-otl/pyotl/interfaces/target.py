from dataclasses import dataclass, field, asdict
from abc import ABC
from enum import Enum
from typing import Any, List, Dict, Literal
from pyotl.interfaces.runtime import RuntimeRequirements
from pyotl.interfaces.paths import OtlFilePath


class OtlTargetType(Enum):
    Test = "test"
    Stimulus = "stimulus"
    Build = "build"


TargetRef = str


@dataclass
class Target(ABC):
    """
    A target is a structure that holds logic to generate a `Command`

    Targets are higher level abstraction to generate a new command based off of certain new input criterea -- for instance, if a target fails, and you create a new command
    """

    name: str
    injected_state: Dict[str, Any] = field(init=False)

    def __post_init__(self):
        self.injected_state = {}

    def get_outputs(self) -> Dict[str, OtlFilePath]:
        return {}

    def gen_script(self) -> List[str]:
        raise NotImplementedError

    @staticmethod
    def rule_type() -> OtlTargetType:
        return OtlTargetType.Test

    def runtime_env_vars(self) -> Dict[str, str]:
        return {}

    def runtime_requirements(self) -> RuntimeRequirements:
        return RuntimeRequirements.default(self.runtime_env_vars())

    def get_dependencies(self) -> List[TargetRef]:
        """
        Returns the targets that this target depends on
        """
        return []

    def get_dependent_files(self) -> List[str]:
        """
        Returns the files that this target depends on

        If any of these files change across invocations, the target will be re-executed
        """
        return []
