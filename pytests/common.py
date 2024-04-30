from dataclasses import replace
from pyotl.rc import OtlRC
from typing import Dict

from pyotl.importer import get_all_targets, DocumentedTarget


def get_test_rc() -> OtlRC:
    default_jobs = 1
    otl_rules_dir = "tests/rules"

    default_rc = OtlRC.default()
    test_rc = replace(
        default_rc, default_jobs=default_jobs, otl_rules_dir=otl_rules_dir
    )

    return test_rc


def get_test_rules() -> Dict[str, DocumentedTarget]:
    otlrc = get_test_rc()
    targets = get_all_targets(otlrc)
    return targets
