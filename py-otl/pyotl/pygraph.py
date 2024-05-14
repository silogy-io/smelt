from typing import Callable, Dict, Generator, List, Optional, Tuple, cast
from pyotl.interfaces import Command, OtlTargetType, Target
from dataclasses import dataclass
from pyotl.otl_muncher import lower_target_to_command, parse_otl
from pyotl.pyotl import PyController, PySubscriber
import yaml
import time


from pyotl.otl_telemetry.data import Event


from pyotl.subscribers.is_done import IsDoneSubscriber
from pyotl.subscribers.output_collector import OutputConsole
from pyotl.subscribers.retcode import RetcodeTracker

from copy import deepcopy

from pyotl.subscribers.stdout import StdoutPrinter, StdoutSink


def default_target_rerun_callback(target: Target, return_code: int) -> Optional[Target]:
    """
    First pass at re-run logic -- currently we just rerun all tests that are tagged as tests

    Power users could supply their own logic, but we should define something that is robust and sane
    """

    if target.rule_type() == OtlTargetType.Test:
        new_target = deepcopy(target)
        new_target.name = f"{new_target.name}_rerun"
        return new_target


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
    PyGraph is the python wrapper for the otl runtime.
    """

    otl_targets: Optional[Dict[str, Target]]
    """ 
    holds the original otl targets (the python rules) that the user supplied. 

    This is used to re-generate new commands on failure
    """
    commands: Dict[str, List[Command]]
    """ 
    holds all of the commands that are live in the graph
    """
    controller: PyController
    listener: PySubscriber
    done_tracker = IsDoneSubscriber()
    retcode_tracker = RetcodeTracker()

    def runloop(self):
        with OutputConsole() as console:
            while not self.done_tracker.is_done:
                # tbh, this could be async
                # but async interopt with rust is kind of experimental and i dont want to do it yet
                message = maybe_get_message(self.listener, blocking=False)
                if message:
                    self.done_tracker.process_message(message)
                    self.retcode_tracker.process_message(message)
                    console.process_message(message)
                if not message:
                    # add a little bit of backoff
                    time.sleep(0.01)

    def console_runloop(
        self, test_name: str, sink: StdoutSink
    ) -> Generator[bool, None, None]:
        stdout_tracker = StdoutPrinter(test_name, sink)
        while True:
            # tbh, this could be async
            # but async interopt with rust is kind of experimental and i dont want to do it yet
            message = maybe_get_message(self.listener, blocking=False)
            if message:
                self.done_tracker.process_message(message)
                self.retcode_tracker.process_message(message)
                stdout_tracker.process_message(message)
            if not message:
                # add a little bit of backoff
                if self.done_tracker.is_done:
                    yield True
                yield False

    def reset(self):
        self.done_tracker.reset()
        self.retcode_tracker.reset()

    def run_one_test(self, name: str):
        self.reset()
        self.controller.run_one_test(name)
        self.runloop()

    def run_one_test_interactive(self, name: str, sink: StdoutSink = print):
        """
        Runs a single test, with a "sink" handle to process all of the stdout for that specific command
        """
        self.reset()
        self.controller.run_one_test(name)
        for is_done in self.console_runloop(name, sink):
            if is_done:
                return
            time.sleep(0.1)

    def run_specific_commands(self, commands: List[Command]):
        self.reset()
        test_names = [command.name for command in commands]
        self.controller.run_many_tests(test_names)
        self.runloop()

    def run_all_tests(self, maybe_type: str):
        self.reset()
        self.controller.run_all_tests(maybe_type)
        self.runloop()

    def rerun(
        self,
        rerun_callback: Callable[
            [Target, int], Optional[Target]
        ] = default_target_rerun_callback,
    ):
        if self.otl_targets:
            new_targets = [
                rerun_callback(self.otl_targets[target], retcode)
                for target, retcode in self.retcode_tracker.retcode_dict.items()
            ]

            filtered_targets = [target for target in new_targets if target]
            for target in filtered_targets:
                self.otl_targets[target.name] = target
            # TODO: its likely that we will also need to handle regenerating dependencies
            #       for a first pass functionality, lets ignore this for now
            command_list = lower_target_to_command(filtered_targets)
            self.add_commands(command_list)
            self.run_specific_commands(command_list)
        else:
            print(
                "Warning! Cannot auto re-run because no otl targets have been provided"
            )

    def add_commands(self, commands: List[Command]):
        for command in commands:
            self.commands[command.target_type].append(command)

        commands_as_str = yaml.safe_dump([command.to_dict() for command in commands])
        self.controller.set_graph(commands_as_str)

    @classmethod
    def init(cls, otl_targets: Dict[str, Target], commands: List[Command]):
        rv = {}
        for tar_typ in OtlTargetType:
            rv[tar_typ.value] = []

        for tar_typ in OtlTargetType:
            rv[tar_typ.value] = []
        for command in commands:
            rv[command.target_type].append(command)

        graph = PyController()

        listener = graph.add_py_listener()
        rv = cls(
            otl_targets=otl_targets, commands=rv, controller=graph, listener=listener
        )
        rv.add_commands(commands)
        return rv

    # This is a testing utility
    @classmethod
    def init_commands_only(cls, commands: List[Command]):
        return cls.init({}, commands)


def create_graph(otl_test_list: str) -> PyGraph:
    targets, command_list = parse_otl(otl_test_list)
    return PyGraph.init(targets, command_list)
