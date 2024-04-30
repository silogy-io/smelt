from dataclasses import dataclass
import yaml
from typing import Dict, Any, Type, List
from pydantic import BaseModel
from pyotl.importer import DocumentedTarget, get_all_targets, get_default_targets
from pyotl.interfaces import Target, Command
from pyotl.rc import OtlRC
from pyotl.path_utils import get_git_root
from pyotl.pygraph import PyGraph


class SerYamlTarget(BaseModel):
    name: str
    rule: str
    rule_args: Dict[str, Any]


@dataclass
class PreTarget:
    target_typ: Type[Target]
    rule_args: Dict[str, Any]


def populate_rule_args(
    target_name: str,
    rule_payload: SerYamlTarget,
    all_rules: Dict[str, DocumentedTarget],
):
    rule_payload.rule_args["name"] = target_name
    if rule_payload.rule not in all_rules:
        # TODO: make a pretty error that
        #
        # Says that no rule is visible
        # Prints out all rules that are visible
        # Point to the location where end users can create new rules
        raise RuntimeError(f"Rule named {rule_payload.rule} has not been created!")
    target_type = all_rules[rule_payload.rule]["target"]
    return PreTarget(target_typ=target_type, rule_args=rule_payload.rule_args)


def to_target(pre_target: PreTarget) -> Target:
    return pre_target.target_typ(**pre_target.rule_args)


def otl_to_command_list(
    test_list: str, rc: OtlRC, default_rules_only: bool = False
) -> List[Command]:
    yaml_content = open(test_list).read()
    return otl_contents_to_command_list(yaml_content, rc, default_rules_only)


def otl_contents_to_command_list(
    otl_content: str, rc: OtlRC, default_rules_only: bool = False
) -> List[Command]:
    rule_inst = yaml.safe_load(otl_content)
    # NOTE: semantically we split up validation of the otl file -> converting to target objects -> generating a command list
    # while dependency based
    if default_rules_only:
        all_rules = get_default_targets(rc)
    else:
        all_rules = get_all_targets(rc)
    yaml_targets = [SerYamlTarget(**target) for target in rule_inst]
    pre_targets = {
        target.name: populate_rule_args(target.name, target, all_rules)
        for target in yaml_targets
    }
    inst_rules = {
        name: to_target(pre_target) for name, pre_target in pre_targets.items()
    }

    command_list = [
        Command.from_target(otl_target, default_root=rc.otl_default_root)
        for otl_target in inst_rules.values()
    ]
    return command_list


def otl_parse(test_list: str, rc: OtlRC) -> PyGraph:
    command_list = otl_to_command_list(test_list, rc)
    return PyGraph.from_command_list(command_list)