from dataclasses import dataclass
import yaml
from typing import Dict, Any, Iterable, Tuple, Type, List
from pydantic import BaseModel
from pyotl.importer import DocumentedTarget, get_all_targets, get_default_targets
from pyotl.interfaces import Target, Command
from pyotl.rc import OtlRC, OtlRcHolder
from pyotl.path_utils import get_git_root


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
) -> PreTarget:
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


def parse_otl(
    test_list: str, default_rules_only: bool = False
) -> Tuple[Dict[str, Target], List[Command]]:
    yaml_content = open(test_list).read()
    targets = otl_contents_to_targets(
        yaml_content, default_rules_only=default_rules_only
    )
    command_list = lower_target_to_command(targets.values())

    return targets, command_list


def lower_target_to_command(targets: Iterable[Target]):

    rc = OtlRcHolder.current_rc
    command_list = [
        Command.from_target(otl_target, default_root=rc.otl_default_root)
        for otl_target in targets
    ]
    return command_list


def otl_contents_to_targets(
    otl_content: str,
    rc: OtlRC = OtlRcHolder.current_rc,
    default_rules_only: bool = False,
) -> Dict[str, Target]:
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
    return {name: to_target(pre_target) for name, pre_target in pre_targets.items()}
