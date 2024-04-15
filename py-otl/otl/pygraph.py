from typing import Dict, List, Tuple
from otl.interfaces import Command, OtlTargetType
from dataclasses import dataclass
from otl.otl import SyncCommandGraph
import yaml


@dataclass
class PyGraph:
    """
    Graph that simply sorts commands by their target type
    """

    targets: Dict[OtlTargetType, List[Command]]
    rsgraph: SyncCommandGraph

    def get_test_type(self, tt: OtlTargetType) -> List[Command]:
        return self.targets[tt]

    @property
    def build(self):
        return self.get_test_type(OtlTargetType.Build)

    @property
    def test(self):
        return self.get_test_type(OtlTargetType.Test)

    @property
    def stimulus(self):
        return self.get_test_type(OtlTargetType.Stimulus)

    def run_one_test(self, name: str):
        self.run_one_test(name)

    def run_all_tests(self, tt: str):
        self.rsgraph.run_all_tests(tt)

    def get_all_tests_as_scripts(self) -> List[Tuple[str, List[str]]]:
        """
        returns test name and script
        """
        return [
            (command.name, command.script)
            for command in self.targets[OtlTargetType.Test]
        ]

    @classmethod
    def from_command_list(cls, commands: List[Command]):
        rv = {}
        for tar_typ in OtlTargetType:
            rv[tar_typ.value] = []
        for command in commands:
            rv[command.target_type].append(command)

        commands_as_str = yaml.safe_dump([command.to_dict() for command in commands])
        graph = SyncCommandGraph(commands_as_str)
        return cls(targets=rv, rsgraph=graph)
