from dataclasses import replace, dataclass
from typing import Dict

import yaml

from pysmelt.importer import get_all_targets, DocumentedTarget
from pysmelt.interfaces import Command, SmeltPath
from pysmelt.path_utils import get_git_root
from pysmelt.pygraph import PyGraph
from pysmelt.rc import SmeltRC


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
    with open(test_list) as f:
        lod = yaml.safe_load(f)
    commands = [Command.from_dict(obj) for obj in lod]
    graph = PyGraph.init_commands_only(commands)
    return graph


@dataclass
class MockRemoteSmeltFileStorage:
    data: Dict[str, str]

    def fetch_smelt_path(self, smelt_path: SmeltPath) -> str:
        return self.data.get(smelt_path.to_abs_path(), "")
