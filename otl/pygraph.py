from typing import Dict, List
from otl.interfaces import Command, OtlTargetType
from dataclasses import dataclass


@dataclass
class PyGraph:
    """
    Naive graph that simply sorts commands by their target type


    """
    targets: Dict[OtlTargetType, List[Command]]

    def get_test_type(self, tt: OtlTargetType) -> List[Command]:
        return self.targets[tt]

    @property
    def build(self):
        return self.get_test_type(OtlTargetType.Simulator)

    @property
    def test(self):
        return self.get_test_type(OtlTargetType.Test)

    @property
    def stimulus(self):
        return self.get_test_type(OtlTargetType.Stimulus)

    @classmethod
    def from_command_list(cls, commands: List[Command]):
        rv = {}
        for tar_typ in OtlTargetType:
            rv[tar_typ] = []
        for command in commands:
            rv[command.target_type].append(command)
        return cls(target=rv)
