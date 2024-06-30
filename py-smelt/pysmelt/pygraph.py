from typing import Callable, Dict, Generator, List, Optional, Tuple, cast

from dataclasses import replace
import betterproto
from pysmelt.interfaces import Command, SmeltTargetType, Target
from dataclasses import dataclass
from pysmelt.interfaces.paths import SmeltPath
from pysmelt.smelt_muncher import SmeltUniverse, create_universe, parse_smelt
from pysmelt.path_utils import relatavize_inp_path
from pysmelt.pysmelt import PyController, PyEventStream
from pysmelt.rc import SmeltRcHolder
from pysmelt.rerun import DerivedTarget, RerunCallback
from pysmelt.subscribers import SmeltSub
import yaml
import time



from pysmelt.proto.smelt_telemetry import Event
from pysmelt.proto.smelt_client.commands import CfgDocker, CfgLocal, ConfigureSmelt


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
    return rv


def default_target_rerun_callback(
    target: Target, return_code: int
) -> Tuple[Target, bool]:
    """
    First pass at re-run logic -- currently we just rerun all tests that are tagged as tests

    Power users could supply their own logic, but we should define something that is robust and sane
    """

    requires_rerun = target.rule_type() == SmeltTargetType.Test and return_code != 0
    
    new_target = replace(target, name=f"{target.name}_rerun")
    new_target.injected_state={"debug": "True"}

    return (new_target, requires_rerun)


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


def spin_for_message(
    listener: PyEventStream, backoff: float = 0.2, time_out: int = 10):
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
    
    retcode_tracker : RetcodeTracker

    additional_listeners: List[SmeltSub]

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
        invbuilder.write_invocation_to_fs()

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

    def run_all_tests(self, maybe_type: str):
        self.reset()
        listener = self.controller.run_all_tests(maybe_type)
        self.runloop(listener)
        

    

    def rerun(
        self,
        rerun_callback: RerunCallback = default_target_rerun_callback,
    ):
        raise NotImplementedError("Currently re-writing this -- please file an issue if this is load bearing!")

    def set_commands(self):
        commands = self.universe.all_commands
        
        
        

        commands_as_str = yaml.safe_dump([command.to_dict() for command in commands])
        self.controller.set_graph(commands_as_str)
        

    @classmethod
    def init(cls, cfg : ConfigureSmelt, universe : SmeltUniverse):
        cfg_bytes = bytes(cfg)
        graph = PyController(cfg_bytes)
        rv = cls(
            universe=universe, controller=graph, retcode_tracker= RetcodeTracker(), additional_listeners=[]
        )
        rv.set_commands()
        return rv

    # This is a testing utility
    @classmethod
    def init_commands_only(cls, commands: List[Command]):
        cfg = default_cfg()
        top_path = SmeltPath.from_str('.')
        universe = SmeltUniverse(top_file=top_path, commands={top_path : commands})
        return cls.init(cfg, universe=universe)

def _create_cfg(smelt_test_list: str) -> ConfigureSmelt:
    command_def_path = SmeltPath.from_str(relatavize_inp_path(SmeltRcHolder.current_rc().smelt_root,smelt_test_list))
    cfg = default_cfg()
    return cfg


def create_graph(smelt_test_list: str, cfg_init: Optional[Callable[[ConfigureSmelt],ConfigureSmelt]] = None) -> PyGraph:
    cfg = _create_cfg(smelt_test_list)
    if cfg_init:
        cfg = cfg_init(cfg)
    universe = create_universe(SmeltPath.from_str(smelt_test_list))
    rv = PyGraph.init(cfg, universe)
    return rv

def create_graph_with_docker(smelt_test_list: str, docker_img: str) -> PyGraph:
    def init_docker(cfg: ConfigureSmelt) -> ConfigureSmelt: 
        cfg.docker = CfgDocker()
        cfg.docker.image_name = docker_img
        cfg.docker.additional_mounts = {}
        return cfg
    return create_graph(smelt_test_list,init_docker)
    
