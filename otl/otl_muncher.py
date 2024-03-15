import yaml
from otl.importer import DocumentedTarget
from typing import Dict


def load_yaml(test_list: str, all_rules: Dict[str, DocumentedTarget]):
    yaml_content = open(test_list).read()
    items = yaml.safe_load(yaml_content)
    print(items)


load_yaml("examples/tests_only.otl", [])
