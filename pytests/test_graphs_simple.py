import subprocess
from typing import Generator
from pysmelt.pygraph import PyGraph, create_graph, create_graph_with_docker
from pysmelt.path_utils import get_git_root
from pysmelt.interfaces import Command


import yaml
import pytest


@pytest.fixture(scope="session")
def simple_docker_image() -> Generator[str, None, None]:
    img = "debian:bookworm-slim"
    subprocess.run(["docker", "pull", img])
    yield img


def test_sanity_pygraph():
    test_list = f"{get_git_root()}/test_data/command_lists/cl1.yaml"
    lod = yaml.safe_load(open(test_list))
    commands = [Command.from_dict(obj) for obj in lod]
    graph = PyGraph.init_commands_only(commands)
    graph.run_all_tests("build")
    graph.run_all_tests("test")


def test_sanity_pygraph_rerun_nofailing():
    """
    Tests the case where no re-run is needed
    """
    test_list = f"{get_git_root()}/test_data/smelt_files/tests_only.smelt.yaml"
    graph = create_graph(test_list)

    graph.run_all_tests("test")
    graph.rerun()
    # we have 3 tests, 0 of which fail -- so we should rerun no tests
    expected_executed_tasks = 3
    observed_reexec = graph.retcode_tracker.total_executed()
    assert observed_reexec == expected_executed_tasks, f"We don't re-run"


def test_sanity_pygraph_rerun_with_failing():
    test_list = f"{get_git_root()}/test_data/smelt_files/failing_tests_only.smelt.yaml"
    graph = create_graph(test_list)
    graph.run_all_tests("test")
    graph.rerun()

    # we have 3 tests, 2 of which fail -- when we re-run, we only run those two
    # 5 total tests total
    expected_failing_tests = 5
    observed_reexec = graph.retcode_tracker.total_executed()

    assert (
        observed_reexec == expected_failing_tests
    ), f"Expected to see {expected_failing_tests} tasks executed, saw {observed_reexec} tests"


def test_sanity_pygraph_new_build():
    test_list = f"{get_git_root()}/test_data/smelt_files/rerun_with_newbuild.smelt.yaml"
    graph = create_graph(test_list)
    graph.run_all_tests("test")
    graph.rerun()

    # we have 3 tests, 2 of which fail
    # BUT we have a debug build that needs to be enabled
    expected_failing_tests = 6
    observed_reexec = graph.retcode_tracker.total_executed()

    assert (
        observed_reexec == expected_failing_tests
    ), f"Expected to see {expected_failing_tests} tasks executed, saw {observed_reexec} tests"


def test_sanity_pygraph_docker(simple_docker_image):
    """
    Tests the case where no re-run is needed
    """
    test_list = f"{get_git_root()}/test_data/smelt_files/tests_only.smelt.yaml"

    graph = create_graph_with_docker(test_list, docker_img=simple_docker_image)

    expected_passed = 3

    graph.run_all_tests("test")
    passed_commands = graph.retcode_tracker.total_passed()
    assert (
        passed_commands == expected_passed
    ), f"Expected to see {expected_passed} tasks passed, saw {passed_commands} tests"
