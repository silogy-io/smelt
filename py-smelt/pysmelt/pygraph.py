import pathlib
import time
from dataclasses import dataclass
from typing import Callable, Generator, List, Optional

import yaml
from pysmelt.pysmelt import PyController, PyEventStream

from pysmelt.interfaces import Command
from pysmelt.interfaces.paths import SmeltPath, SmeltPathFetcher
from pysmelt.proto.smelt_client.commands import (
    CfgDocker,
    CfgLocal,
    ConfigureSmelt,
    ProfilerCfg,
    ProfilingSelection,
)
from pysmelt.proto.smelt_telemetry import Event
from pysmelt.rc import SmeltRcHolder
from pysmelt.smelt_muncher import SmeltUniverse, create_universe
from pysmelt.subscribers import SmeltSub
from pysmelt.subscribers.error_handler import SmeltErrorHandler
from pysmelt.subscribers.invocation_builder import InvocationBuilder
from pysmelt.subscribers.output_collector import OutputConsole
from pysmelt.subscribers.retcode import RetcodeTracker
from pysmelt.subscribers.stdout import StdoutPrinter, StdoutSink


def default_cfg() -> ConfigureSmelt:
    rv = ConfigureSmelt()
    rc = SmeltRcHolder.current_rc()

    rv.job_slots = rc.jobs
    rv.smelt_root = rc.smelt_root
    rv.local = CfgLocal()
    rv.prof_cfg = ProfilerCfg()
    # sample memory
    rv.prof_cfg.prof_type = ProfilingSelection.SIMPLE_PROF
    # sample every 100 ms
    rv.prof_cfg.sampling_period = 100
    return rv


def maybe_get_message(
    listener: PyEventStream, blocking: bool = False
) -> Optional[Event]:
    try:
        if blocking:
            message = listener.pop_message_blocking()
            event = Event.FromString(message)
        else:
            message = listener.nonblocking_pop()
            if message is None:
                return None
            event = Event.FromString(message)

        return event
    except RuntimeError as e:
        """
        In expectation, we only return an error if the channel has been closed

        We don't have any paths above this function to handle that error, so we'll handle it here
        """

        print(e)
        return None


def spin_for_message(listener: PyEventStream, backoff: float = 0.2, time_out: int = 10):
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
    universe: SmeltUniverse
    """ 
    holds all of the commands that are live in the graph 
    """
    controller: PyController
    """
    Handle to rust -- calls the function defined in the pyo3 bindings
    """

    retcode_tracker: RetcodeTracker

    additional_listeners: List[SmeltSub]

    def commands(self):
        return self.universe.top_level_commands

    def runloop(self, listener: PyEventStream):
        errhandler = SmeltErrorHandler()
        invbuilder = InvocationBuilder()
        with OutputConsole() as console:
            while not listener.is_done():
                # tbh, this could be async
                # but async interopt with rust is kind of experimental and i dont want to do it yet
                message = maybe_get_message(listener, blocking=False)
                if message:
                    self.retcode_tracker.process_message(message)
                    console.process_message(message)
                    errhandler.process_message(message)
                    invbuilder.process_message(message)
                    for other_listener in self.additional_listeners:
                        other_listener.process_message(message)
                if not message:
                    # add a little bit of backoff
                    time.sleep(0.01)
        invbuilder.write_invocation_and_junit()

    def console_runloop(
        self, test_name: str, listener: PyEventStream, sink: StdoutSink
    ) -> Generator[bool, None, None]:
        """
        Generator that will try to consume as many `Event` messages as possible

        All of the stdout from the "test_name" command will be given to the `sink`.
        By default, sink should just be print

        The yielded value will be
        """
        stdout_tracker = StdoutPrinter(test_name, sink)
        errhandler = SmeltErrorHandler()
        while True:
            # tbh, this could be async
            # but async interopt with rust is kind of experimental and i dont want to do it yet
            message = maybe_get_message(listener, blocking=False)
            if message:
                self.retcode_tracker.process_message(message)
                stdout_tracker.process_message(message)
                errhandler.process_message(message)
                for other_listener in self.additional_listeners:
                    other_listener.process_message(message)

            if not message:
                # add a little bit of backoff
                yield listener.is_done()

    def get_current_cfg(self) -> ConfigureSmelt:
        raw_cfg = self.controller.get_current_cfg()
        return ConfigureSmelt.FromString(raw_cfg)

    def reset(self):
        pass

    # self.retcode_tracker.reset()

    def run_one_test_interactive(self, name: str, sink: StdoutSink = print):
        """
        Runs a single test, with a "sink" handle to process all of the stdout for that specific command

        By default, this will just print stdout + stderr to the screen -- it looks like you're running the command interactively
        """
        self.reset()
        listener = self.controller.run_one_test(name)
        for is_done in self.console_runloop(name, listener, sink):
            if is_done:
                return
            time.sleep(0.1)

    def run_specific_commands(self, commands: List[Command]):
        self.reset()
        test_names = [command.name for command in commands]
        listener = self.controller.run_many_tests(test_names)
        self.runloop(listener)

    def run_all_typed_commands(self, maybe_type: str):
        self.reset()
        listener = self.controller.run_all_tests(maybe_type)
        self.runloop(listener)

    def run_all_commands(self):
        self.reset()
        toptests = self.universe.top_level_commands
        valid_commands = [
            command.name
            for command in toptests
            if command.target_type != "rebuild" and command.target_type != "rerun"
        ]
        listener = self.controller.run_many_tests(valid_commands)
        self.runloop(listener)

    def set_commands(self):
        """
        Initializes the list of commands that are visible to the smelt runtime


        If the list of commands are malformed -- e.g. syntax error in the yaml, then an error will be thrown
        """
        commands = self.universe.all_commands
        commands_as_str = yaml.safe_dump([command.to_dict() for command in commands])
        command_def_dir = str(pathlib.Path(self.universe.top_file.to_abs_path()).parent)
        self.controller.set_graph(commands_as_str, command_def_dir)

    @classmethod
    def init(cls, cfg: ConfigureSmelt, universe: SmeltUniverse):
        cfg_bytes = bytes(cfg)
        graph = PyController(cfg_bytes)
        rv = cls(
            universe=universe,
            controller=graph,
            retcode_tracker=RetcodeTracker(),
            additional_listeners=[],
        )
        rv.set_commands()
        return rv

    @classmethod
    def init_commands_only(cls, commands: List[Command]):
        cfg = default_cfg()
        top_path = SmeltPath.from_str(".")
        universe = SmeltUniverse(top_file=top_path, commands={top_path: commands})
        return cls.init(cfg, universe=universe)


def _create_cfg() -> ConfigureSmelt:
    cfg = default_cfg()
    return cfg


def create_graph(
    smelt_test_list: str,
    cfg_init: Optional[Callable[[ConfigureSmelt], ConfigureSmelt]] = None,
    default_rules_only: bool = False,
    file_fetcher: Optional[SmeltPathFetcher] = None,
) -> PyGraph:
    cfg = _create_cfg()
    if cfg_init:
        cfg = cfg_init(cfg)
    universe = create_universe(
        SmeltPath.from_str(smelt_test_list),
        default_rules_only=default_rules_only,
        file_fetcher=file_fetcher,
    )
    rv = PyGraph.init(cfg, universe)
    return rv


def create_graph_with_docker(smelt_test_list: str, cfg_docker: CfgDocker) -> PyGraph:
    def init_docker(cfg: ConfigureSmelt) -> ConfigureSmelt:
        cfg.docker = cfg_docker
        return cfg

    return create_graph(smelt_test_list, cfg_init=init_docker)
