from typing import Dict, Generator, List, Optional, Tuple, cast
from pyotl.interfaces import Command, OtlTargetType, Target
from dataclasses import dataclass
from pyotl.otl_muncher import lower_targets_to_commands, parse_otl
from pyotl.path_utils import get_git_root, memoize
from pyotl.pyotl import PyController, PySubscriber
from pyotl.rc import OtlRcHolder
from pyotl.rerun import DerivedTarget, RerunCallback
import yaml
import time
import os



from pyotl.otl_telemetry.data import Event
from pyotl.otl_client.commands import CfgDocker, CfgLocal, ConfigureOtl


from pyotl.subscribers.error_handler import OtlErrorHandler
from pyotl.subscribers.is_done import IsDoneSubscriber
from pyotl.subscribers.output_collector import OutputConsole
from pyotl.subscribers.retcode import RetcodeTracker

from copy import deepcopy

from pyotl.subscribers.stdout import StdoutPrinter, StdoutSink


@memoize
def default_cfg() -> ConfigureOtl:
    rv = ConfigureOtl()
    rc = OtlRcHolder.current_rc()

    rv.job_slots = rc.jobs
    rv.otl_root = rc.otl_root
    rv.local = CfgLocal()

    return rv


def default_target_rerun_callback(
    target: Target, return_code: int
) -> Tuple[Target, bool]:
    """
    First pass at re-run logic -- currently we just rerun all tests that are tagged as tests

    Power users could supply their own logic, but we should define something that is robust and sane
    """

    requires_rerun = target.rule_type() == OtlTargetType.Test and return_code != 0
    new_target = deepcopy(target)
    new_target.name = f"{new_target.name}_rerun"
    new_target.injected_state = {"debug": "True"}
    return (new_target, requires_rerun)


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


def spin_for_message(
    listener: PySubscriber, backoff: float = 0.2, time_out: int = 10):
    """
    Utility for spinning for a message 
    """
    time_passed = 0 
    while time_passed < time_out:
        message = maybe_get_message(listener, blocking=False)
        if message:
            return message
        else:
            time.sleep(backoff) 
            time_passed += backoff

    raise RuntimeError(f"Before timeout of {time_out} expired!")



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
    commands: List[Command]
    """ 
    holds all of the commands that are live in the graph -- some of these may not map back to an otl target
    """
    controller: PyController
    listener: PySubscriber
    done_tracker : IsDoneSubscriber
    retcode_tracker : RetcodeTracker

    def runloop(self):
        errhandler = OtlErrorHandler()
        with OutputConsole() as console:
            while not self.done_tracker.is_done:
                # tbh, this could be async
                # but async interopt with rust is kind of experimental and i dont want to do it yet
                message = maybe_get_message(self.listener, blocking=False)
                if message:
                    self.done_tracker.process_message(message)
                    self.retcode_tracker.process_message(message)
                    console.process_message(message)
                    errhandler.process_message(message)
                if not message:
                    # add a little bit of backoff
                    time.sleep(0.01)

    def console_runloop(
        self, test_name: str, sink: StdoutSink
    ) -> Generator[bool, None, None]:
        """
        Generator that will try to consume as many `Event` messages as possible

        All of the stdout from the "test_name" command will be given to the `sink`.
        By default, sink should just be print

        The yielded value will be
        """
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
                yield self.done_tracker.is_done

    def reset(self):
        self.done_tracker.reset()
       # self.retcode_tracker.reset()
        

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
        rerun_callback: RerunCallback = default_target_rerun_callback,
    ):
        if self.otl_targets:
            """
            We create a dictionary that maps all of the target names to a "DerivedTarget", that holds all of the state for prospective targets that may need to be created 

            This dictionary maps the original target name -> DerivedTarget bundle
            """
            all_derived = {
                target: DerivedTarget.from_cb(
                    orig_target, rerun_callback(self.otl_targets[target], retcode)
                )
                for target, retcode in self.retcode_tracker.retcode_dict.items()
                if (target in self.otl_targets and (orig_target := self.otl_targets[target]))
            }



            """
            For each target, we go through and see if any of the "derived" targets have "changed" from the original target 

            a target changing can mean one of three things

            1. it needs to be rerun 
            2. the command contents has changed and it does not need to be re-run (for example, when a if we want to have a debug build
            3. A dependency of the target has changed

            If a derived target has changed from its original, we lower it to a command, correct its dependencies and it is returned as part of the new commands list
            """

            new_commands = [
                (new_target, target.requires_rerun)
                for target in all_derived.values()
                if (new_target := target.get_new_command(all_derived)) is not None
            ]
            if not any(new_commands):
                return "No new commands were produced"
            all_commands_to_add, requires_rerun = map(list, zip(*new_commands))
            all_commands_to_add = cast(List[Command], all_commands_to_add)
            requires_rerun = cast(List[bool], requires_rerun)
            
            if not any(requires_rerun):
                print("No commands need a rerun is required!")
                return

            # TODO: its likely that we will also need to handle regenerating dependencies
            #       for a first pass functionality, lets ignore this for now

            self.add_commands(all_commands_to_add)
            commands_to_run = [
                command
                for command, rerun in new_commands 
                if rerun 
            ]
            self.run_specific_commands(commands_to_run)
        else:
            print(
                "Warning! Cannot auto re-run because no otl targets have been provided"
            )

    def add_commands(self, commands: List[Command]):
        self.commands += commands
        commands_as_str = yaml.safe_dump([command.to_dict() for command in commands])
        self.controller.set_graph(commands_as_str)
        message = spin_for_message(self.listener)
        OtlErrorHandler().process_message(message)

    @classmethod
    def init(cls, otl_targets: Dict[str, Target], commands: List[Command], cfg : ConfigureOtl = default_cfg()):
        cfg_bytes = bytes(cfg)
        graph = PyController(cfg_bytes)
        listener = graph.add_py_listener()
        rv = cls(
            otl_targets=otl_targets, commands=[], controller=graph, listener=listener, done_tracker=IsDoneSubscriber(), retcode_tracker= RetcodeTracker()
        )
        rv.add_commands(commands)
        return rv

    # This is a testing utility
    @classmethod
    def init_commands_only(cls, commands: List[Command]):
        return cls.init({}, commands)


def create_graph(otl_test_list: str, cfg : ConfigureOtl = default_cfg()) -> PyGraph:
    targets, command_list = parse_otl(otl_test_list)
    return PyGraph.init(targets, command_list, cfg)

def create_graph_with_docker(otl_test_list: str, docker_img: str) -> PyGraph:
    cfg = default_cfg()
    cfg.docker = CfgDocker()
    cfg.docker.image_name = docker_img
    cfg.docker.additional_mounts = {}
    return create_graph(otl_test_list)
