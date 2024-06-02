from dataclasses import replace
from pysmelt.rc import SmeltRC
from typing import Dict

from pysmelt.importer import get_all_targets, DocumentedTarget
import pytest
import subprocess
from typing import Generator
import pytest

from pysmelt.pygraph import PyGraph, create_graph, create_graph_with_docker
from pysmelt.path_utils import get_git_root
from pysmelt.interfaces import Command


import yaml


def get_test_rc() -> SmeltRC:
    default_jobs = 1
    smelt_rules_dir = "tests/rules"

    default_rc = SmeltRC.default()
    test_rc = replace(
        default_rc, default_jobs=default_jobs, smelt_rules_dir=smelt_rules_dir
    )

    return test_rc


def get_test_rules() -> Dict[str, DocumentedTarget]:
    smeltrc = get_test_rc()
    targets = get_all_targets(smeltrc)
    return targets


def create_command_list_graph(cl_name: str) -> PyGraph:
    test_list = f"{get_git_root()}/test_data/command_lists/{cl_name}"
    lod = yaml.safe_load(open(test_list))
    commands = [Command.from_dict(obj) for obj in lod]
    graph = PyGraph.init_commands_only(commands)
    return graph
