from dataclasses import dataclass, field, asdict
from abc import ABC
from enum import Enum
from functools import partial
from typing import Any, List, Dict, Literal, Optional, TypedDict
from pysmelt.interfaces.runtime import RuntimeRequirements
from pysmelt.interfaces.paths import SmeltFilePath
from pysmelt.rc import SmeltRcHolder


class SmeltTargetType(Enum):
    Test = "test"
    Stimulus = "stimulus"
    Build = "build"
    ## not to be used by end users
    Rebuild = "rebuild"
    Rerun = "rerun"


class CGVar(Enum):
    base = "base"
    rerun = "rerun"
    rebuild = "rebuild"


TargetRef = str


smelt_target = partial(dataclass, frozen=True)()


NamedFiles = Dict[str, str]
"""
Named files are a collection of names mapping to SmeltPaths

We use str instead of the SmeltPath in the value to keep implimentation simple

"""


@dataclass
class Target(ABC):
    """
    A target is a structure that holds logic to generate a `Command`

    Targets are higher level abstraction to commands -- they allow users to embed "application logic" into targets

    """

    name: str

    @property
    def ws_path(self) -> str:
        return f"$SMELT_ROOT/smelt-out/{self.name}"

    def get_outputs(self, command_gen_type: CGVar = CGVar.base) -> NamedFiles:
        return {}

    def gen_script(self) -> List[str]:
        raise NotImplementedError

    def gen_rebuild_script(self) -> Optional[List[str]]:
        return None

    def gen_rerun_script(self) -> Optional[List[str]]:
        return None

    @staticmethod
    def rule_type() -> SmeltTargetType:
        return SmeltTargetType.Test

    def runtime_requirements(
        self, command_gen_type: CGVar = CGVar.base
    ) -> RuntimeRequirements:
        return RuntimeRequirements.default()

    def get_dependencies(self, command_gen_type: CGVar = CGVar.base) -> List[TargetRef]:
        """
        Returns the targets that this target depends on
        """
        return []

    def get_dependent_files(self, command_gen_type: CGVar = CGVar.base) -> List[str]:
        """
        Returns the files that this target depends on
        """
        return []

    @property
    def as_ref(self) -> TargetRef:
        """
        Currently refs are just the names of each target
        """
        return self.name
