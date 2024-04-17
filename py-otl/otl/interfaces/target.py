from dataclasses import dataclass
from abc import ABC
from enum import Enum
from typing import List, Dict, Literal
from otl.interfaces.runtime import RuntimeRequirements
from otl.interfaces.paths import OtlPath
from otl.path_utils import get_git_root


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
        raise NotImplementedError

    @staticmethod
    def rule_type() -> OtlTargetType:
        return OtlTargetType.Test

    def required_runtime_env_vars(self, default_path: str) -> Dict[str, str]:
        try:
            git_root = get_git_root()
        except Exception:
            git_root = "$(git rev-parse --show-toplevel)"
        otl_root = f"{git_root}/{default_path}"
        target_root = f"{otl_root}/{self.name}"

        return dict(GIT_ROOT=git_root, OTL_ROOT=otl_root, TARGET_ROOT=target_root)

    def runtime_env_vars(self) -> Dict[str, str]:
        return {}

    def runtime_requirements(self) -> RuntimeRequirements:
        return RuntimeRequirements.default(self.runtime_env_vars())

    def dependencies(self) -> List[TargetRef]:
        return []
