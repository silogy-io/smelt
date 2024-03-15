import importlib
import inspect
from typing import List
from otl.interfaces.target import Target


def get_all_targets(target_paths: List[str]):
    default_target_modules = ["otl.default_targets"]
    classes = {}
    base_class_name = 'Target'
    for path in target_paths + default_target_modules:
        try:
            module = importlib.import_module(path)
            for name, cls in inspect.getmembers(module, inspect.isclass):
                if issubclass(cls, Target):
                    if name in classes:
                        raise ValueError(f"Duplicate target name: {name}")
                    if name != base_class_name:
                        classes[name] = cls
                        classes[name] = {
                            'class': cls,
                            'doc': inspect.getdoc(cls)
                        }

        except ImportError:
            print(f"Failed to import rule definitions at {path}")

    print(classes)


get_all_targets([])
