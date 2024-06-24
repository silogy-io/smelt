from pathlib import Path
from typing import List
from pysmelt.importer import import_procedural_testlist
from pysmelt.interfaces.target import Target
import contextlib
import weakref


@contextlib.contextmanager
def capture_targets():
    instances: List[Target] = []

    original_init = Target.__post_init__

    def new_init(self: Target, *args, **kwargs):
        original_init(self, *args, **kwargs)
        instances.append(self)

    Target.__post_init__ = new_init

    yield instances

    Target.__post_init__ = original_init

    return


def get_procedural_targets(py_path: str) -> List[Target]:

    with capture_targets() as a:
        mod = import_procedural_testlist(py_path)

    return a
