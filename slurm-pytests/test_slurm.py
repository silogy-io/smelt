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
    ServerInfo,
    ProfilerCfg,
    ProfilingSelection,
    CfgDocker,
    Ulimit,
    RunMode,
)
from pysmelt.pygraph import PyGraph, create_graph, create_graph_with_docker
from pysmelt.pysmelt import spawn_slurm_server


def test_simple_slurm():

    test_list = f"{get_git_root()}/test_data/smelt_files/tests_only.smelt.yaml"
    slurm_port = 4040
    spawn_slurm_server(slurm_port, True)

    def init_slurm(cfg: ConfigureSmelt) -> ConfigureSmelt:
        cfg.slurm = CfgSlurm()
        cfg.slurm.none = True
        cfg.slurm.maybe_info = ServerInfo(hostname=get_ip_address(), port=slurm_port)
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


import socket


def get_ip_address():
    hostname = socket.gethostname()
    ip_address = socket.gethostbyname(hostname)
    return ip_address


def test_sealed_slurm():
    """ """
    test_list = f"test_data/smelt_files/simple_graph.smelt.yaml"
    img = "test_sealed_slurm_img"
    slurm_port = 9004
    spawn_slurm_server(slurm_port, True)

    # create_sealed("sealed_example", img, test_list)

    def init_slurm(cfg: ConfigureSmelt) -> ConfigureSmelt:
        cfg.test_only = True
        cfg.slurm = CfgSlurm()
        cfg.slurm.dockerws = DockerWorkspace(
            container_name=img, workspace_smelt_root="/opt"
        )
        # TODO: we need to investigate having this unset -- currently it breaks things, unfortunately, because ip 0.0.0.0 is given to
        cfg.slurm.maybe_info = ServerInfo(hostname=get_ip_address(), port=slurm_port)
        return cfg

    graph = create_graph(test_list, cfg_init=init_slurm)

    expected_tests_failed = 2
    observed_failed = graph.retcode_tracker.total_failed()

    assert (
        observed_failed == expected_tests_failed
    ), f"Expected to see {expected_tests_failed} tasks executed, saw {observed_failed} tests"


spawn_slurm_server(4040, True)
print("well done son")
# test_sealed_slurm()
