from dataclasses import dataclass
import functools
import yaml
import pathlib
from typing import ClassVar, Dict, Any, Iterable, Set, Tuple, Type, List
from pydantic import BaseModel
from pysmelt.generators.procedural import get_procedural_targets
from pysmelt.importer import (
    DocumentedTarget,
    get_all_targets,
    get_default_targets,
)
from pysmelt.interfaces import Target, Command
from pysmelt.interfaces.paths import SmeltPath, TempTarget
from pysmelt.interfaces.target import TargetRef
from pysmelt.rc import SmeltRC, SmeltRcHolder
from pysmelt.path_utils import get_git_root


class SerYamlTarget(BaseModel):
    name: str
    rule: str
    rule_args: Dict[str, Any] = {}


@dataclass
class PreTarget:
    target_typ: Type[Target]
    rule_args: Dict[str, Any]


@dataclass
class ImportTracker:
    imported_commands: ClassVar[Dict[SmeltPath, List[Command]]] = {}

    @staticmethod
    def clear():
        ImportTracker.imported_commands = {}

    @staticmethod
    def get_all_imported() -> Dict[SmeltPath, List[Command]]:
        return ImportTracker.imported_commands


def populate_rule_args(
    target_name: str,
    rule_payload: SerYamlTarget,
    all_rules: Dict[str, DocumentedTarget],
) -> PreTarget:
    rule_payload.rule_args["name"] = target_name
    if rule_payload.rule not in all_rules:
        print(all_rules)

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


def get_targets(
    test_list: SmeltPath, default_rules_only: bool = False
) -> Dict[str, Target]:

    test_list2 = test_list.to_abs_path()

    if pathlib.Path(test_list2).suffix == ".py":
        targets = get_procedural_targets(test_list2)

        return {target.name: target for target in targets}

    else:
        yaml_content = open(test_list2).read()
        return smelt_contents_to_targets(
            yaml_content, default_rules_only=default_rules_only
        )


def parse_smelt(
    test_list: SmeltPath, default_rules_only: bool = False
) -> Tuple[Dict[str, Target], List[Command]]:
    test_list_orig = test_list
    targets = get_targets(test_list, default_rules_only)
    command_list = lower_targets_to_commands(
        targets.values(), str(pathlib.Path(test_list_orig.to_abs_path()).parent)
    )

    return targets, command_list


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
    # Initialize the top file, seen files, visible files, and all commands
    top_file = starting_file
    seen_files = {starting_file}
    visible_files = {starting_file}
    all_commands = {}

    # Parse the "initial" file under consideration and all of the testlists seen to visible files
    _, commands = parse_smelt(starting_file, default_rules_only)
    for comm in commands:
        for dep in comm.dependencies:
            tt = TempTarget.parse_string_smelt_target(dep, starting_file.to_abs_path())
            visible_files.add(tt.file_path)
    all_commands[top_file] = commands
    all_commands.update(ImportTracker.imported_commands)
    ImportTracker.clear()

    # Determine new files that are visible but not yet parsed
    new_files = visible_files - seen_files

    # Continue to parse new files until we've seen everything
    while True:
        if len(new_files) != 0:
            file = new_files.pop()
            seen_files.add(file)
            _, new_commands = parse_smelt(
                file,
                default_rules_only,
            )

            # Add dependencies of new commands to new files if not seen
            for command in new_commands:
                for dep in command.dependencies:
                    tt = TempTarget.parse_string_smelt_target(
                        dep, starting_file.to_abs_path()
                    )
                    if tt.file_path not in seen_files:
                        new_files.add(tt.file_path)

            all_commands[file] = new_commands
            all_commands.update(ImportTracker.imported_commands)
            ImportTracker.clear()

        else:
            # Break the loop if there are no new files
            break

    # Should have everything in the Universe now
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
