from dataclasses import dataclass
import yaml
import pathlib
from typing import Dict, Any, Iterable, Set, Tuple, Type, List
from pydantic import BaseModel
from pysmelt.generators.procedural import get_procedural_targets
from pysmelt.importer import (
    DocumentedTarget,
    get_all_targets,
    get_default_targets,
    import_procedural_testlist,
)
from pysmelt.interfaces import Target, Command
from pysmelt.interfaces.paths import SmeltPath
from pysmelt.interfaces.target import TargetRef
from pysmelt.rc import SmeltRC, SmeltRcHolder
from pysmelt.path_utils import get_git_root


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


def parse_smelt(
    test_list: SmeltPath, default_rules_only: bool = False
) -> Tuple[Dict[str, Target], List[Command]]:
    test_list_orig = test_list
    test_list2 = test_list.to_abs_path()
    if pathlib.Path(test_list2).suffix == ".py":
        targets = get_procedural_targets(test_list2)

        targets = {target.name: target for target in targets}

    else:
        yaml_content = open(test_list2).read()
        targets = smelt_contents_to_targets(
            yaml_content, default_rules_only=default_rules_only
        )
    command_list = lower_targets_to_commands(targets.values(), test_list_orig.path)

    return targets, command_list


@dataclass(frozen=True)
class TempTarget:
    name: str
    file_path: SmeltPath

    @classmethod
    def parse_string_smelt_target(cls, raw_target: TargetRef, current_file: str):
        # Split the string on the colon
        if raw_target.startswith("//"):
            parts = raw_target.split(":")

            # Check if the split resulted in exactly two parts
            if len(parts) != 2:
                raise ValueError("TargetRef was formatted incorrectly")

            # The first part is the path
            path = parts[0]

            # Remove the leading '//' from the path
            if path.startswith("//"):
                path = path[2:]

            if pathlib.Path(path).suffix == ".py":
                raise RuntimeError(
                    "targets declared in python test lists are currently not supported as dependencies!"
                )

            # The second part is the target name
            target_name = parts[1]

            return cls(name=target_name, file_path=SmeltPath.from_str(path))
        else:
            target_name = raw_target
            return cls(name=target_name, file_path=SmeltPath.from_str(current_file))


@dataclass
class SmeltUniverse:
    top_file: SmeltPath
    commands: Dict[SmeltPath, List[Command]]

    @property
    def all_commands(self) -> List[Command]:
        return [
            command
            for command_list in self.commands.values()
            for command in command_list
        ]


def create_universe(
    starting_file: SmeltPath,
    default_rules_only: bool = False,
) -> SmeltUniverse:
    top_file = starting_file
    seen_files = {starting_file}
    visible_files = set()
    all_commands = {}
    _, commands = parse_smelt(starting_file, default_rules_only)
    for comm in commands:
        for dep in comm.dependencies:
            tt = TempTarget.parse_string_smelt_target(dep, starting_file.to_abs_path())
            visible_files.add(tt.file_path)
    all_commands[top_file] = commands
    # all smelt_files
    new_files = seen_files - visible_files
    while True:
        if len(new_files) != 0:
            file = new_files.pop()
            seen_files.add(file)
            _, new_commands = parse_smelt(
                file,
                default_rules_only,
            )

            for command in new_commands:
                for dep in command.dependencies:
                    tt = TempTarget.parse_string_smelt_target(
                        dep, starting_file.to_abs_path()
                    )
                    if tt.file_path not in seen_files:
                        new_files.add(tt.file_path)

            all_commands[file] = new_commands
        else:
            break

    return SmeltUniverse(top_file=top_file, commands=all_commands)


def target_to_command(target: Target, working_dir: str) -> Command:
    rc = SmeltRcHolder.current_rc()
    return Command.from_target(target, working_dir)


def lower_targets_to_commands(targets: Iterable[Target], path: str) -> List[Command]:
    return [target_to_command(target, path) for target in targets]


def smelt_contents_to_targets(
    smelt_content: str,
    rc: SmeltRC = SmeltRcHolder.current_rc(),
    default_rules_only: bool = False,
) -> Dict[str, Target]:
    rule_inst = yaml.safe_load(smelt_content)
    # NOTE: semantically we split up validation of the smelt file -> converting to target objects -> generating a command list
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
