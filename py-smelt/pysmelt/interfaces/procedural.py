from dataclasses import dataclass
from typing import ClassVar, Dict, List
from pysmelt.generators.procedural import dec_import_depth, inc_import_depth
from pysmelt.interfaces.command import Command
from pysmelt.interfaces.paths import SmeltPath, TempTarget
from pysmelt.interfaces.target import Target
from pysmelt.smelt_muncher import parse_smelt
from pysmelt.tracker import ImportTracker


def import_as_target(target_path: str) -> Target:
    inc_import_depth()
    tt = TempTarget.parse_string_smelt_target(
        target_path, ImportTracker.local_file_alias().path
    )
    chosen_name = tt.name
    if tt.file_path not in ImportTracker.imported_commands:
        targets, commands = parse_smelt(tt.file_path)
        # ImportTracker.imported_commands[tt.file_path] = commands
        if chosen_name not in targets:
            raise RuntimeError(f"Could not find command {target_path}")
    dec_import_depth()
    return ImportTracker.imported_targets[tt.file_path][chosen_name]
