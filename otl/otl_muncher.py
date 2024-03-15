import yaml


def parse_yaml(path_to_otl: str) -> None:
    yaml_content = open(path_to_otl).read()
    yaml.safe_load(yaml_content)
