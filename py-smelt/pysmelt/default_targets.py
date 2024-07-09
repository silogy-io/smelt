from dataclasses import dataclass, field
from functools import partial
from pysmelt.interfaces import Target, SmeltFilePath, SmeltTargetType, TargetRef
from typing import Any, List, Dict, Optional

from pysmelt.interfaces.runtime import RuntimeRequirements


@dataclass
class raw_bash(Target):
    """
    Simple target for embedding raw bash commands in Smelt

    Environment variables available, to all targets are:
        * ${SMELT_ROOT}: the root of the smelt-workspace -- by default, this will be ${GIT_ROOT}
        * ${TARGET_ROOT}: the working space of the current command -- it will be ${SMELT_ROOT}/smelt-out/${COMMAND_NAME}
    """

    cmds: List[str] = field(default_factory=list)
    deps: List[TargetRef] = field(default_factory=list)
    outputs: Dict[str, str] = field(default_factory=dict)
    debug_cmds: Optional[List[str]] = None
    rebuild_cmds: Optional[List[str]] = None
    num_cpus: Optional[int] = None
    timeout: Optional[int] = None
    mem_usage: Optional[int] = None

    def gen_script(self) -> List[str]:
        return self.cmds

    def gen_rerun_script(self) -> Optional[List[str]]:
        return self.debug_cmds

    def gen_rebuild_script(self) -> Optional[List[str]]:
        return self.rebuild_cmds

    def get_dependencies(self) -> List[TargetRef]:
        return self.deps

    def get_outputs(
        self,
    ) -> Dict[str, str]:
        return self.outputs

    def runtime_requirements(
        self,
    ) -> RuntimeRequirements:
        rr = RuntimeRequirements.default()
        if self.num_cpus:
            rr.num_cpus = self.num_cpus
        if self.timeout:
            rr.timeout = self.timeout
        return rr


@dataclass
class raw_bash_build(raw_bash):
    @staticmethod
    def rule_type() -> SmeltTargetType:
        return SmeltTargetType.Build


@dataclass
class test_group(Target):
    """
    Target for oragnizing tests
    """

    tests: List[TargetRef] = field(default_factory=list)

    def gen_script(self) -> List[str]:
        return [f"echo {test}" for test in self.tests]

    def get_dependencies(self) -> List[TargetRef]:
        return self.tests
