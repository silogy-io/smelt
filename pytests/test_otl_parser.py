from pyotl.rc import OtlRC
from pyotl.otl_muncher import parse_otl


from pyotl.path_utils import get_git_root


def test_sanity_otl_parse():

    test_list = f"{get_git_root()}/examples/tests_only.otl"
    _, commands = parse_otl(test_list=test_list, default_rules_only=True)
    assert len(commands) == 3, "Didn't parse out every command"
