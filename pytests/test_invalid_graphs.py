from pyotl.pygraph import PyGraph, create_graph, create_graph_with_docker
from pyotl.path_utils import get_git_root
from pyotl.interfaces import Command


import yaml
import pytest

from pytests.common import create_command_list_graph


def test_missing_dep_graph():
    with pytest.raises(RuntimeError) as e_info:
        create_command_list_graph(cl_name="cl_invalid_nodep.yaml")


def test_missing_file_dep_graph():
    with pytest.raises(RuntimeError) as e_info:
        create_command_list_graph(cl_name="cl_invalid_missing_file_dep.yaml")


def test_invalid_yaml_file():
    with pytest.raises(RuntimeError) as e_info:
        create_command_list_graph(cl_name="cl_invalid.yaml")


def test_dup_filename():
    with pytest.raises(RuntimeError) as e_info:
        create_command_list_graph(cl_name="cl_invalid_double_names.yaml")


def test_dup_outputs():
    with pytest.raises(RuntimeError) as e_info:
        create_command_list_graph(cl_name="cl_invalid_double_output.yaml")
