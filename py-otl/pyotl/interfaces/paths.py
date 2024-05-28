from enum import Enum
from dataclasses import dataclass
from pathlib import Path


class OtlPathType(Enum):
    Absolute = 1
    OtlRootRelative = 2
    OtlCommandDefRelative = 3


@dataclass
class OtlPath:
    path_type: OtlPathType
    path: str

    @property
    def as_str(self):
        return self.path

    @classmethod
    def abs_path(cls, path: str):
        return cls(path_type=OtlPathType.Absolute, path=path)

    @classmethod
    def from_str(cls, path: str):
        if Path(path).is_absolute():
            path_type = OtlPathType.Absolute
        else:
            path_type = OtlPathType.OtlRootRelative
        return cls(path_type=path_type, path=path)

    def __str__(self):
        return self.path

    def to_abs_path(self, otl_root: str):
        if self.path_type == OtlPathType.OtlRootRelative:
            return f"${otl_root}/{self.path}"
        elif self.path_type == OtlPathType.Absolute:
            return f"{self.path}"
        else:
            raise NotImplementedError(f"Unhandled variant {self.path_type}")


@dataclass
class OtlFilePath:
    path_type: OtlPathType
    path: str

    @classmethod
    def abs_path(cls, path: str):
        return cls(path_type=OtlPathType.Absolute, path=path)

    @classmethod
    def from_str(cls, path: str):
        if Path(path).is_absolute():
            path_type = OtlPathType.Absolute
        else:
            path_type = OtlPathType.OtlCommandDefRelative
        return cls(path_type=path_type, path=path)

    def __str__(self):
        return self.path

    def to_abs_path(self, abs_cmd_path: str):
        if self.path_type == OtlPathType.OtlRootRelative:
            return f"${abs_cmd_path}/{self.path}"
        elif self.path_type == OtlPathType.Absolute:
            return f"{self.path}"
        else:
            raise NotImplementedError(f"Unhandled variant {self.path_type}")
