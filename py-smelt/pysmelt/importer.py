import importlib
import inspect
from typing import List, TypedDict, Dict, Type, Optional
from pysmelt.interfaces import Target
from pathlib import Path
from pysmelt.interfaces.command import Command
from pysmelt.interfaces.paths import (
    TempTarget,
    SmeltPathFetcher,
    local_smelt_path_fetcher,
)
from pysmelt.rc import SmeltRC
from pysmelt.path_utils import get_git_root
import sys
from importlib.util import spec_from_file_location, module_from_spec


class DocumentedTarget(TypedDict):
    target: Type[Target]
    doc: str


def get_all_files(targets_dir: Path) -> List[Path]:
    rules_files = []
    if targets_dir.exists() and targets_dir.is_dir():
        rule_files = [
            maybe_target_file for maybe_target_file in targets_dir.glob("**/*.py")
        ]
        return rule_files

    return rules_files


def get_all_targets(cfg: SmeltRC) -> Dict[str, DocumentedTarget]:
    rules_dir: Path = cfg.abs_rules_dir
    return _get_all_targets(rules_dir)


def get_default_targets(cfg: SmeltRC) -> Dict[str, DocumentedTarget]:
    return _get_all_targets(None)


def _get_all_targets(targets_dir: Optional[Path]) -> Dict[str, DocumentedTarget]:
    default_target_modules = ["pysmelt.default_targets"]
    classes = {}
    base_class_name = "Target"

    if targets_dir:
        all_paths = get_all_files(targets_dir=targets_dir)
        paths = default_target_modules + all_paths
    else:
        paths = default_target_modules

    for path in paths:
        try:

            if isinstance(path, str):
                module = importlib.import_module(str(path))
            elif isinstance(path, Path):
                module_name = path.stem  # get filename without extension
                spec = spec_from_file_location(module_name, str(path))
                module = module_from_spec(spec)
                spec.loader.exec_module(module)

            for name, cls in inspect.getmembers(module, inspect.isclass):
                if issubclass(cls, Target):
                    if name in classes:
                        raise ValueError(f"Duplicate target name: {name}")
                    if name != base_class_name:
                        classes[name] = cls
                        classes[name] = {"target": cls, "doc": inspect.getdoc(cls)}

        except ImportError as e:
            print(f"Failed to import rule definitions at {path} with error {e}")

    return classes


def import_procedural_testlist(py_path: str):
    spec = spec_from_file_location("__main__", py_path)

    module = module_from_spec(spec)

    spec.loader.exec_module(module)

    return module
