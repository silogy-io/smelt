from dataclasses import dataclass, field, asdict
from abc import ABC
from enum import Enum
from functools import partial
from typing import Any, List, Dict, Literal
from pysmelt.interfaces.runtime import RuntimeRequirements
from pysmelt.interfaces.paths import SmeltFilePath
from pysmelt.rc import SmeltRcHolder


class SmeltTargetType(Enum):
    Test = "test"
    Stimulus = "stimulus"
    Build = "build"


TargetRef = str


smelt_target = partial(dataclass, frozen=True)()


@dataclass
class Target(ABC):
    """
    A target is a structure that holds logic to generate a `Command`

    Targets are higher level abstraction to commands -- they allow users to embed "application logic" into targets

    """

    name: str
    injected_state: Dict[str, Any] = field(init=False)

    @property
    def ws_path(self) -> str:
        return f"$SMELT_ROOT/smelt-out/{self.name}"

    def __post_init__(self):
        self.injected_state = {}

    def get_outputs(self) -> Dict[str, str]:
        return {}

    def gen_script(self) -> List[str]:
        raise NotImplementedError

    @staticmethod
    def rule_type() -> SmeltTargetType:
        return SmeltTargetType.Test

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

    @property
    def as_ref(self) -> TargetRef:
        """
        Currently refs are just the names of each target
        """
        return self.name
