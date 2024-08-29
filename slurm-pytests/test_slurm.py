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
    ProfilerCfg,
    ProfilingSelection,
    CfgDocker,
    Ulimit,
    RunMode,
)
from pysmelt.pygraph import PyGraph, create_graph, create_graph_with_docker




def try_make_slurm():

    test_list = f"{get_git_root()}/test_data/smelt_files/tests_only.smelt.yaml"

    def init_slurm(cfg: ConfigureSmelt) -> ConfigureSmelt:
        cfg.slurm = CfgSlurm()
        return cfg

    graph = create_graph(test_list, cfg_init=init_slurm)
    graph.run_all_typed_commands("test")
                                                                                       
    expected_tests = 3
    observed_reexec = graph.retcode_tracker.total_executed()
                                                                                       
    assert (
        observed_reexec == expected_tests
    ), f"Expected to see {expected_tests} tasks executed, saw {observed_reexec} tests" 
try_make_slurm()
