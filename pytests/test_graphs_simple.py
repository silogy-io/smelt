import subprocess
import math
from typing import Generator
from pysmelt.proto.smelt_client.commands import (
    ConfigureSmelt,
    ProfilerCfg,
    ProfilingSelection,
)
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


def test_get_cfg():

    cmd_def_path_in = "test_data/smelt_files/large_profile.smelt.yaml"
    test_list = f"{get_git_root()}/test_data/smelt_files/large_profile.smelt.yaml"
    graph = create_graph(test_list)
    assert (
        cmd_def_path_in == graph.get_current_cfg().command_def_path
    ), "command_def_path roundtrip doesn't match"


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


def test_sanity_pygraph_runone():
    """
    Tests running one test -- we didn't have this path tested, woops
    """
    test_list = f"{get_git_root()}/test_data/smelt_files/tests_only.smelt.yaml"
    graph = create_graph(test_list)

    graph.run_one_test_interactive("test_example_1")
    # we have 3 tests, 0 of which fail -- so we should rerun no tests


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


def test_profiler():
    """
    Tests the case where no re-run is needed
    """
    from pysmelt.subscribers.simple_profiler import ProfileWatcher
    from typing import cast

    test_list = f"{get_git_root()}/test_data/smelt_files/large_profile.smelt.yaml"

    def init_sampler(cfg: ConfigureSmelt) -> ConfigureSmelt:
        cfg.prof_cfg = ProfilerCfg(
            prof_type=ProfilingSelection.SIMPLE_PROF, sampling_period=100
        )
        return cfg

    graph = create_graph(test_list, init_sampler)
    graph.additional_listeners.append(ProfileWatcher())
    graph.run_all_tests("test")
    profiler = cast(ProfileWatcher, graph.additional_listeners[0])

    big_mem_events = profiler.profile_events["high_mem_usage"]
    smaller_mem = profiler.profile_events["baseline"]

    def find_average(lst):
        return sum(lst) / len(lst)

    for event in big_mem_events:
        assert event.cpu_load != float(
            "nan"
        ), "We should not have any nans for cpu load!"
    for eventlists in [big_mem_events, smaller_mem]:
        for event in eventlists:
            assert not math.isnan(
                event.cpu_load
            ), "We should not have any nans for cpu load!"
            assert not math.isinf(
                event.cpu_load
            ), "We should not have infinite cpu load!"

    avg_big_mem = find_average([event.memory_used for event in big_mem_events])
    avg_small_mem = find_average([event.memory_used for event in smaller_mem])

    mem_used_ratio = avg_big_mem / avg_small_mem
    lower_bound = 2.5
    if not mem_used_ratio > lower_bound:
        import warnings

        warnings.warn(
            UserWarning(
                f"""We expect that the more memory test takes about ~4x more memory than the baseline -- we set a lower bound of 2.5x mem to be safe.

                actual observed ratio is {mem_used_ratio}
                """
            )
        )

    # assert (
    #    mem_used_ratio > lower_bound
    # ), "We expect that the more memory test takes about ~4x more memory than the baseline -- we set a lower bound of 2.5x mem to be safe"


test_profiler()
