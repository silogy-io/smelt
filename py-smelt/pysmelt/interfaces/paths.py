from enum import Enum
from dataclasses import dataclass
from pathlib import Path


class SmeltPathType(Enum):
    Absolute = 1
    SmeltRootRelative = 2
    SmeltCommandDefRelative = 3


@dataclass
class SmeltPath:
    path_type: SmeltPathType
    path: str

    @property
    def as_str(self):
        return self.path

    @classmethod
    def abs_path(cls, path: str):
        return cls(path_type=SmeltPathType.Absolute, path=path)

    @classmethod
    def from_str(cls, path: str):
        if Path(path).is_absolute():
            path_type = SmeltPathType.Absolute
        else:
            path_type = SmeltPathType.SmeltRootRelative
        return cls(path_type=path_type, path=path)

    def __str__(self):
        return self.path

    def to_abs_path(self, smelt_root: str):
        if self.path_type == SmeltPathType.SmeltRootRelative:
            return f"${smelt_root}/{self.path}"
        elif self.path_type == SmeltPathType.Absolute:
            return f"{self.path}"
        else:
            raise NotImplementedError(f"Unhandled variant {self.path_type}")


@dataclass
class SmeltFilePath:
    """
    A file object in smelt can either be absolute or defined relative to the command_def path

    for instance, if we have an output "foo"

    """

    path_type: SmeltPathType
    path: str

    @classmethod
    def abs_path(cls, path: str):
        return cls(path_type=SmeltPathType.Absolute, path=path)

    @classmethod
    def from_str(cls, path: str):
        if Path(path).is_absolute():
            path_type = SmeltPathType.Absolute
        else:
            path_type = SmeltPathType.SmeltCommandDefRelative
        return cls(path_type=path_type, path=path)

    def __str__(self):
        return self.path

    def to_abs_path(self, abs_cmd_path: str):
        if self.path_type == SmeltPathType.SmeltRootRelative:
            return f"${abs_cmd_path}/{self.path}"
        elif self.path_type == SmeltPathType.Absolute:
            return f"{self.path}"
        else:
            raise NotImplementedError(f"Unhandled variant {self.path_type}")
