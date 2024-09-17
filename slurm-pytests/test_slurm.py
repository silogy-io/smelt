import math
import subprocess
from tempfile import NamedTemporaryFile, TemporaryDirectory
from typing import Generator

import pytest
import yaml

from pysmelt.interfaces import Command
from pysmelt.path_utils import get_git_root
from pysmelt.proto.smelt_client.commands import (
    CfgSlurm,
    ConfigureSmelt,
    DockerWorkspace,
    ProfilerCfg,
    ProfilingSelection,
    CfgDocker,
    Ulimit,
    RunMode,
)
from pysmelt.pygraph import PyGraph, create_graph, create_graph_with_docker


def test_simple_slurm():

    test_list = f"{get_git_root()}/test_data/smelt_files/tests_only.smelt.yaml"

    def init_slurm(cfg: ConfigureSmelt) -> ConfigureSmelt:
        cfg.slurm = CfgSlurm()
        cfg.slurm.none = True
        return cfg

    graph = create_graph(test_list, cfg_init=init_slurm)
    graph.run_all_typed_commands("test")

    expected_tests = 3
    observed_reexec = graph.retcode_tracker.total_executed()

    assert (
        observed_reexec == expected_tests
    ), f"Expected to see {expected_tests} tasks executed, saw {observed_reexec} tests"


def create_sealed(container_name: str, committed_img_name: str, smelt_file_path: str):
    """
    Smelt file path should always be relative to root
    """
    root = get_git_root()
    bash_script_path = f"{root}/test_utils/create_sealed.sh"
    smelt_file_path = f"{smelt_file_path}"

    subprocess.run(
        [bash_script_path, container_name, committed_img_name, smelt_file_path]
    )


def test_sealed_slurm():
    """ """
    test_list = f"test_data/smelt_files/simple_graph.smelt.yaml"
    img = "test_sealed_slurm_img"

    create_sealed("smelt_dev", img, test_list)

    def init_slurm(cfg: ConfigureSmelt) -> ConfigureSmelt:
        cfg.test_only = True
        cfg.slurm = CfgSlurm()
        cfg.slurm.dockerws = DockerWorkspace(
            container_name=img, workspace_smelt_root="/src"
        )
        return cfg

    graph = create_graph(test_list, cfg_init=init_slurm)
    graph.run_all_typed_commands("test")

    expected_tests_failed = 2
    observed_failed = graph.retcode_tracker.total_failed()

    assert (
        observed_failed == expected_tests_failed
    ), f"Expected to see {expected_tests_failed} tasks executed, saw {observed_failed} tests"
