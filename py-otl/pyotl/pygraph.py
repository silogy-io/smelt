from typing import Dict, List, Optional, Tuple
from pyotl.interfaces import Command, OtlTargetType
from dataclasses import dataclass
from pyotl.pyotl import PyController, PySubscriber
import yaml
import time


from pyotl.otl_telemetry.data import Event
import betterproto

from pyotl.subscribers.is_done import IsDoneSubscriber


def maybe_get_message(
    listener: PySubscriber, blocking: bool = False
) -> Optional[Event]:
    if blocking:
        message = listener.pop_message_blocking()
        event = Event.FromString(message)
    else:
        message = listener.nonblocking_pop()
        if message is None:
            return None
        event = Event.FromString(message)
    return event


@dataclass
class PyGraph:
    """
    Graph that simply sorts commands by their target type
    """

    targets: Dict[OtlTargetType, List[Command]]
    controller: PyController
    listener: PySubscriber
    done_tracker = IsDoneSubscriber()

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
        self.done_tracker.reset()
        self.controller.run_one_test(name)
        while not self.done_tracker.is_done:
            message = maybe_get_message(self.listener, blocking=True)
            if message:
                self.done_tracker.process_message(message)

    def run_all_tests(self, maybe_type: str):
        self.done_tracker.reset()
        handle = self.controller.run_all_tests(maybe_type)
        while not self.done_tracker.is_done:
            message = maybe_get_message(self.listener, blocking=True)
            if message:
                self.done_tracker.process_message(message)

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
        graph = PyController()
        graph.set_graph(commands_as_str)
        listener = graph.add_py_listener()
        return cls(targets=rv, controller=graph, listener=listener)
