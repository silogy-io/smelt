from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import sys
from typing import List
from pysmelt.importer import import_procedural_testlist


from pysmelt.interfaces.target import Target


import contextlib


from pysmelt.path_utils import get_git_root

_import_depth = 0


def get_import_depth() -> int:
    return _import_depth


def inc_import_depth():
    global _import_depth
    _import_depth += 1


def dec_import_depth():
    global _import_depth
    _import_depth -= 1


@contextlib.contextmanager
def capture_targets():
    instances: List[Target] = []

    original_init = Target.__post_init__

    def new_init(self: Target, *args, **kwargs):
        original_init(self, *args, **kwargs)
        if _import_depth == 0:
            instances.append(self)

    Target.__post_init__ = new_init

    yield instances

    Target.__post_init__ = original_init

    return


def get_procedural_targets(py_path: str) -> List[Target]:

    with capture_targets() as a:
        mod = import_procedural_testlist(py_path)

    return a


def init_local_rules():
    root_dir = Path(get_git_root()) / "smelt_rules"
    if root_dir.exists():
        for py_file in root_dir.glob("*.py"):
            module_name = py_file.stem  # get filename without extension
            spec = spec_from_file_location(module_name, str(py_file))
            module = module_from_spec(spec)
            spec.loader.exec_module(module)
            sys.modules[module_name] = (
                module  # add the module to the list of globally imported modules
            )
