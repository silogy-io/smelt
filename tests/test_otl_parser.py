
from otl.rc import OtlRC
from otl.otl_muncher import otl_to_command_list

from .common import get_test_rules

from otl.path_utils import get_git_root


def test_sanity_otl_parse():
    targets = get_test_rules()
    test_list = f"{get_git_root()}/examples/tests_only.otl"
    commands = otl_to_command_list(
        test_list=test_list, all_rules=targets, rc=OtlRC.default())
    assert len(commands) == 3, "Didn't parse out every command"
