from pyotl.pygraph import PyGraph, create_graph
from pyotl.path_utils import get_git_root
from pyotl.interfaces import Command
import yaml


def test_sanity_pygraph():
    test_list = f"{get_git_root()}/test_data/command_lists/cl1.yaml"
    lod = yaml.safe_load(open(test_list))
    commands = [Command.from_dict(obj) for obj in lod]
    graph = PyGraph.init_commands_only(commands)
    graph.run_all_tests("build")
    graph.run_all_tests("test")


def test_sanity_pygraph_rerun():
    test_list = f"{get_git_root()}/examples/tests_only.otl"
    graph = create_graph(test_list)
    graph.run_all_tests("test")
    graph.rerun()


test_sanity_pygraph_rerun()
