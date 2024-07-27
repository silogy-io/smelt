from dataclasses import dataclass, field, asdict
from abc import ABC
from enum import Enum
from functools import partial
from typing import Any, List, Dict, Literal, Optional, TypedDict
from pysmelt.interfaces.command import Command
from pysmelt.interfaces.runtime import RuntimeRequirements
from pysmelt.interfaces.paths import SmeltFilePath
from pysmelt.rc import SmeltRcHolder
from pysmelt.tracker import ImportTracker, try_get_target


class SmeltTargetType(Enum):
    Test = "test"
    Stimulus = "stimulus"
    Build = "build"
    ## not to be used by end users
    Rebuild = "rebuild"
    Rerun = "rerun"


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

    def get_outputs(
        self,
    ) -> NamedFiles:
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
        self,
    ) -> RuntimeRequirements:
        return RuntimeRequirements.default()

    def get_dependencies(
        self,
    ) -> List[TargetRef]:
        """
        Returns the targets that this target depends on
        """
        return []

    def get_dependent_files(
        self,
    ) -> List[str]:
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

    def _default_to_command(self, working_dir: str) -> Command:
        name = self.name
        target_type = self.rule_type().value
        script = self.gen_script()
        runtime = self.runtime_requirements()
        dependencies = self.get_dependencies()
        dependent_files = self.get_dependent_files()
        rerun_command = self.to_rerun_command(working_dir)
        outputs = list(map(lambda path: str(path), self.get_outputs().values()))
        return Command(
            name=name,
            target_type=target_type,
            script=script,
            runtime=runtime,
            dependencies=dependencies,
            dependent_files=dependent_files,
            outputs=outputs,
            working_dir=working_dir,
            on_failure=f"{rerun_command.name}" if rerun_command else None,
        )

    def to_command(self, working_dir: str) -> Command:
        return self._default_to_command(working_dir)

    def to_rerun_command(self, working_dir: str) -> Optional[Command]:
        return self.default_rerun_command(working_dir)

    def to_rebuild_command(self, working_dir: str) -> Optional[Command]:
        return self.default_rebuild_command(working_dir)

    def default_rerun_command(self, working_dir: str) -> Optional[Command]:
        script = self.gen_rerun_script()
        if script:
            name = f"{self.name}--rerun"
            target_type = SmeltTargetType.Rerun.value
            runtime = self.runtime_requirements()
            dependencies = [
                rebuild_command.name
                for dep in (
                    try_get_target(f"{idep}") for idep in self.get_dependencies()
                )
                if dep is not None
                for rebuild_command in [dep.to_rebuild_command(working_dir)]
                if rebuild_command is not None
            ]

            dependent_files = self.get_dependent_files()
            outputs = list(map(lambda path: str(path), self.get_outputs().values()))
            return Command(
                name=name,
                target_type=target_type,
                script=script,
                runtime=runtime,
                dependencies=dependencies,
                dependent_files=dependent_files,
                outputs=outputs,
                working_dir=working_dir,
            )

    def default_rebuild_command(self, working_dir: str) -> Optional[Command]:
        script = self.gen_rebuild_script()
        if script:
            name = f"{self.name}--rebuild"
            target_type = SmeltTargetType.Rebuild.value
            runtime = self.runtime_requirements()
            dependencies = [
                rebuild_command.name
                for dep in (
                    try_get_target(f"{idep}") for idep in self.get_dependencies()
                )
                if dep is not None
                for rebuild_command in [dep.to_rebuild_command(working_dir)]
                if rebuild_command is not None
            ]

            dependent_files = self.get_dependent_files()
            outputs = list(map(lambda path: str(path), self.get_outputs().values()))
            return Command(
                name=name,
                target_type=target_type,
                script=script,
                runtime=runtime,
                dependencies=dependencies,
                dependent_files=dependent_files,
                outputs=outputs,
                working_dir=working_dir,
            )

    def __post_init__(self):
        pass
