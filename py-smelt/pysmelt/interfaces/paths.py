from enum import Enum
from dataclasses import dataclass
from pathlib import Path
import pathlib
from typing import Optional

from pysmelt import rc


class SmeltPathType(Enum):
    Absolute = 1
    SmeltRootRelative = 2
    SmeltCommandDefRelative = 3


@dataclass(frozen=True)
class SmeltPath:
    path: str

    @property
    def as_str(self):
        return self.path

    @classmethod
    def from_str(cls, path: str):
        return cls(path=path)

    def __str__(self):
        return self.path

    @staticmethod
    def translate(in_path: str) -> str:
        return SmeltPath.from_str(in_path).to_abs_path()

    def to_abs_path(self):
        smelt_root = rc.SmeltRcHolder.current_smelt_root()
        if Path(self.path).is_absolute():
            return f"{self.path}"
        else:
            return f"{smelt_root}/{self.path}"


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
            return f"{abs_cmd_path}/{self.path}"
        elif self.path_type == SmeltPathType.Absolute:
            return f"{self.path}"
        else:
            raise NotImplementedError(f"Unhandled variant {self.path_type}")


@dataclass(frozen=True)
class TempTarget:
    name: str
    file_path: SmeltPath

    @classmethod
    def parse_string_smelt_target(
        cls, raw_target: str, current_file: Optional[str] = None
    ):
        # Split the string on the colon
        if raw_target.startswith("//"):

            parts = raw_target.split(":")

            # Check if the split resulted in exactly two parts
            if len(parts) != 2:
                raise ValueError("TargetRef was formatted incorrectly")

            # The first part is the path
            path = parts[0]

            # Remove the leading '//' from the path
            if path.startswith("//"):
                path = path[2:]

            if pathlib.Path(path).suffix == ".py":
                pass

            # The second part is the target name
            target_name = parts[1]
            path = SmeltPath.from_str(path)
            if not pathlib.Path(path.to_abs_path()).exists:
                raise RuntimeError(f"path does not exist {path.to_abs_path()}")

            return cls(name=target_name, file_path=path)
        else:
            assert current_file
            target_name = raw_target
            return cls(name=target_name, file_path=SmeltPath.from_str(current_file))
