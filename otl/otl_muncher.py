from dataclasses import dataclass
import yaml
from typing import Dict, Any, Type, List
from pydantic import BaseModel
from otl.importer import DocumentedTarget, get_all_targets
from otl.interfaces import Command, Target
from otl.rc import OtlRC


class SerYamlTarget(BaseModel):
    name: str
    rule: str
    rule_args: Dict[str, Any]


@dataclass
class PreTarget:
    target_typ: Type[Target]
    rule_args: Dict[str, Any]


def populate_rule_args(
        target_name: str, rule_payload: SerYamlTarget, all_rules: Dict[str, DocumentedTarget]
):

    rule_payload.rule_args["name"] = target_name
    if rule_payload.rule not in all_rules:
        # TODO: make a pretty error that
        #
        # Says that no rule is visible
        # Prints out all rules that are visible
        # Point to the location where end users can create new rules
        raise RuntimeError(
            f"Rule named {rule_payload.target} has not been created!")
    target_type = all_rules[rule_payload.rule]['target']
    return PreTarget(target_typ=target_type, rule_args=rule_payload.rule_args)


def to_target(pre_target: PreTarget) -> Target:
    return pre_target.target_typ(**pre_target.rule_args)


def otl_to_command_list(test_list: str, all_rules: Dict[str, DocumentedTarget]) -> List[Command]:
    yaml_content = open(test_list).read()
    rule_inst = yaml.safe_load(yaml_content)
    # NOTE: semantically we split up validation of the otl file -> converting to target objects -> generating a command list
    # while dependency based
    yaml_targets = [SerYamlTarget(**target) for target in rule_inst]
    pre_targets = {
        target.name: populate_rule_args(target.name, target, all_rules) for target in yaml_targets
    }
    inst_rules = {
        name: to_target(pre_target) for name, pre_target in pre_targets.items()
    }

    command_list = [Command.from_target(otl_target)
                    for otl_target in inst_rules.values()]
    return command_list
