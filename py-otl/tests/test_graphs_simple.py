from otl.pygraph import PyGraph
from otl.path_utils import get_git_root
from otl.interfaces import Command
import yaml


def test_sanity_pygraph():
    test_list = f"{get_git_root()}/test_data/command_lists/cl1.yaml"
    lod = yaml.safe_load(open(test_list))
    commands = [Command.from_dict(obj) for obj in lod]
    graph = PyGraph.from_command_list(commands)
    graph.run_all_tests("build")
    graph.run_all_tests("test")
