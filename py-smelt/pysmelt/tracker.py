from dataclasses import dataclass

from typing import ClassVar, Dict, List, Optional


from typing import TYPE_CHECKING

from pysmelt.interfaces.paths import SmeltPath, TempTarget


if TYPE_CHECKING:

    from pysmelt.interfaces.command import Command
    from pysmelt.interfaces.target import Target, TargetRef


@dataclass
class ImportTracker:
    imported_commands: ClassVar[Dict[SmeltPath, List["Command"]]] = {}
    imported_targets: ClassVar[Dict[SmeltPath, Dict[str, "Target"]]] = {}

    @staticmethod
    def clear():
        ImportTracker.imported_commands = {}

    @staticmethod
    def clear_all():
        ImportTracker.imported_commands = {}

    @staticmethod
    def local_file_alias():
        return SmeltPath("local")

    @staticmethod
    def get_all_imported() -> Dict[SmeltPath, List["Command"]]:
        return ImportTracker.imported_commands


def try_get_target(target: "TargetRef") -> Optional["Target"]:
    tt = TempTarget.parse_string_smelt_target(
        target, ImportTracker.local_file_alias().path
    )
    try:
        return ImportTracker.imported_targets[tt.file_path][tt.name]
    except Exception as _:
        return None
