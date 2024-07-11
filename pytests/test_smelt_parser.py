from pysmelt.rc import SmeltRC
from pysmelt.smelt_muncher import parse_smelt


from pysmelt.path_utils import get_git_root
from pysmelt.interfaces.paths import SmeltPath


def test_sanity_smelt_parse():

    test_list = f"{get_git_root()}/test_data/smelt_files/tests_only.smelt.yaml"
    _, commands = parse_smelt(
        test_list=SmeltPath.from_str(test_list), default_rules_only=True
    )
    assert len(commands) == 3, "Didn't parse out every command"
