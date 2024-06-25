from pysmelt.interfaces.paths import TempTarget
from pysmelt.interfaces.target import Target
from pysmelt.smelt_muncher import parse_smelt


def import_as_target(target_path: str) -> Target:
    tt = TempTarget.parse_string_smelt_target(target_path)
    tt.file_path.to_abs_path()
    chosen_name = tt.name
    targets, _ = parse_smelt(tt.file_path)
    if chosen_name not in targets:
        raise RuntimeError(f"Could not find command {target_path}")
    return targets[chosen_name]
