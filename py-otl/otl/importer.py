import importlib
import inspect
from typing import List, TypedDict, Dict, Type
from otl.interfaces import Target
from pathlib import Path
from otl.rc import OtlRC


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


def get_all_targets(cfg: OtlRC) -> Dict[str, DocumentedTarget]:
    rules_dir: Path = cfg.abs_rules_dir
    return _get_all_targets(rules_dir)


def _get_all_targets(targets_dir: Path) -> Dict[str, DocumentedTarget]:
    default_target_modules = ["otl.default_targets"]
    classes = {}
    base_class_name = "Target"

    all_paths = get_all_files(targets_dir=targets_dir)
    for path in default_target_modules + all_paths:
        try:
            module = importlib.import_module(str(path))
            for name, cls in inspect.getmembers(module, inspect.isclass):
                if issubclass(cls, Target):
                    if name in classes:
                        raise ValueError(f"Duplicate target name: {name}")
                    if name != base_class_name:
                        classes[name] = cls
                        classes[name] = {"target": cls, "doc": inspect.getdoc(cls)}

        except ImportError:
            print(f"Failed to import rule definitions at {path}")

    return classes
