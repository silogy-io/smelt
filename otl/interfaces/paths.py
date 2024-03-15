from enum import Enum
from dataclasses import dataclass


class OtlPathType(Enum):
    TargetRelative = 1
    GitRelative = 2
    Absolute = 3
    OtlRootRelative = 4


@dataclass
class OtlPath:
    path_type: OtlPathType
    path: str

    @classmethod
    def abs_path(cls, path: str):
        return cls(path_type=OtlPathType.Absolute, path=path)

    @classmethod
    def target_relative(cls, path: str):
        return cls(path_type=OtlPathType.TargetRelative, path=path)

    @classmethod
    def git_relative(cls, path: str):
        return cls(path_type=OtlPathType.GitRelative, path=path)

    def __str__(self):
        return self.to_string()

    def to_string(self):
        if self.path_type == OtlPathType.GitRelative:
            return f"${{GIT_ROOT}}/{self.path}"
        elif self.path_type == OtlPathType.TargetRelative:
            return f"${{TARGET_ROOT}}/{self.path}"
        elif self.path_type == OtlPathType.OtlRootRelative:
            return f"${{OTL_ROOT}}/{self.path}"
        elif self.path_type == OtlPathType.Absolute:
            return f"{self.path}"
        else:
            raise NotImplementedError(f"Unhandled variant {self.path_type}")
