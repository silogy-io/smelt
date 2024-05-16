from dataclasses import dataclass, field
from pyotl.interfaces import Target, OtlPath, OtlTargetType, TargetRef
from typing import List, Dict


@dataclass
class raw_bash(Target):
    """
    Simple target for embedding raw bash commands in Otl

    Environment variables avaible are:
        * ${GIT_ROOT}: the git root of the current git workspace
        * ${OTL_ROOT}: the root of the otl-workspace -- by default, this will be ${GIT_ROOT}/otl
        * ${TARGET_ROOT}: the working space of the current target
    """

    cmds: List[str] = field(default_factory=list)
    debug_cmds: List[str] = field(default_factory=list)
    dependencies: List[TargetRef] = field(default_factory=list)

    def gen_script(self) -> List[str]:
        if "debug" in self.injected_state and self.debug_cmds:
            return self.debug_cmds
        else:
            return self.cmds

    def get_dependencies(self) -> List[TargetRef]:
        return self.dependencies


@dataclass
class raw_bash_build(raw_bash):
    @staticmethod
    def rule_type() -> OtlTargetType:
        return OtlTargetType.Build
