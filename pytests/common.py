from dataclasses import replace
from pyotl.rc import OtlRC
from typing import Dict

from pyotl.importer import get_all_targets, DocumentedTarget
import pytest
import subprocess
from typing import Generator
import pytest

from pyotl.pygraph import PyGraph, create_graph, create_graph_with_docker
from pyotl.path_utils import get_git_root
from pyotl.interfaces import Command


import yaml


def get_test_rc() -> OtlRC:
    default_jobs = 1
    otl_rules_dir = "tests/rules"

    default_rc = OtlRC.default()
    test_rc = replace(
        default_rc, default_jobs=default_jobs, otl_rules_dir=otl_rules_dir
    )

    return test_rc


def get_test_rules() -> Dict[str, DocumentedTarget]:
    otlrc = get_test_rc()
    targets = get_all_targets(otlrc)
    return targets


def create_command_list_graph(cl_name: str) -> PyGraph:
    test_list = f"{get_git_root()}/test_data/command_lists/{cl_name}"
    lod = yaml.safe_load(open(test_list))
    commands = [Command.from_dict(obj) for obj in lod]
    graph = PyGraph.init_commands_only(commands)
    return graph
