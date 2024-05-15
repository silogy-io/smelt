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


def test_sanity_pygraph_rerun_nofailing():
    test_list = f"{get_git_root()}/test_data/otl_files/tests_only.otl"
    graph = create_graph(test_list)
    graph.run_all_tests("test")
    graph.rerun()
    # we have 3 tests, 0 of which fail -- so we should rerun no tests
    expected_executed_tasks = 3
    observed_reexec = graph.retcode_tracker.total_executed()
    assert observed_reexec == expected_executed_tasks, f"We don't re-run"


def test_sanity_pygraph_rerun_with_failing():
    test_list = f"{get_git_root()}/test_data/otl_files/failing_tests_only.otl"
    graph = create_graph(test_list)
    graph.run_all_tests("test")
    graph.rerun()

    # we have 3 tests, 2 of which fail -- when we re-run, we only run those two
    expected_failing_tests = 2
    observed_reexec = graph.retcode_tracker.total_executed()

    # assert (
    #    observed_reexec == expected_failing_tests
    # ), f"Expecteted to see {expected_failing_tests} tasks executed, saw {observed_reexec} tests"


test_sanity_pygraph_rerun_with_failing()
