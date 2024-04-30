from pyotl.rc import OtlRC
from pyotl.otl_muncher import otl_to_command_list


from pyotl.path_utils import get_git_root


def test_sanity_otl_parse():

    test_list = f"{get_git_root()}/examples/tests_only.otl"
    commands = otl_to_command_list(
        test_list=test_list, rc=OtlRC.default(), default_rules_only=True
    )
    assert len(commands) == 3, "Didn't parse out every command"
