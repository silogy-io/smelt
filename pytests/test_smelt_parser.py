from pysmelt.rc import SmeltRC
from pysmelt.smelt_muncher import parse_smelt


from pysmelt.path_utils import get_git_root


def test_sanity_smelt_parse():

    test_list = f"{get_git_root()}/examples/tests_only.smelt"
    _, commands = parse_smelt(test_list=test_list, default_rules_only=True)
    assert len(commands) == 3, "Didn't parse out every command"
