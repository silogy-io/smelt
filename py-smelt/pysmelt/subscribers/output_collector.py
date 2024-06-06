from datetime import datetime
import enum
from typing import Dict, Optional
from typing_extensions import cast
from rich.console import Group
from rich.tree import Tree
import rich
from pysmelt.interfaces.target import SmeltTargetType, Target
from pysmelt.output import smelt_console
from dataclasses import dataclass, field
from pysmelt.proto.smelt_telemetry import (
    CommandEvent,
    CommandFinished,
    CommandStdout,
    Event,
)
import betterproto
from rich.progress import (
    Progress,
    RenderableColumn,
    SpinnerColumn,
    Task,
    TaskID,
    TimeElapsedColumn,
)


class Status(enum.Enum):
    scheduled = "scheduled"
    started = "started"
    finished = "finished"
    cancelled = "cancelled"


@dataclass
class StatusObj:
    name: str
    started_time: datetime
    command_type: SmeltTargetType
    status: Status


class RenderableTree(RenderableColumn):
    """Renders completed filesize."""

    def render(self, task: Task):
        """Show data completed."""
        status_dict: Dict[str, Status] = task.fields["fields"]["status"]
        root = Tree("Running tasks")
        for command, status in status_dict.items():
            if status == Status.started:
                sub1 = root.add(f"Running {command}")
        return root


@dataclass
class OutputConsole:
    """
    Simple subscriber for creating a console
    """

    print_stdout: bool = False
    is_done: bool = False
    total_run: int = 0
    total_executing: int = 0
    total_passed: int = 0
    status_dict: Dict[str, Status] = field(default_factory=dict)
    progress: Optional[Progress] = None
    task: Optional[TaskID] = None

    def __enter__(self):

        self.progress = Progress(
            RenderableTree(),
            SpinnerColumn(),
            TimeElapsedColumn(),
        )

        self.progress.start()
        self.task = self.progress.add_task(
            "Executing smelt...", fields={"status": self.status_dict}
        )

        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.progress:
            self.progress.stop()
            self.progress = None
        smelt_console.log(
            f"Executed {self.total_run} tasks, {self.total_passed} tasks passed"
        )
        pass

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "command":
            event_payload: CommandEvent
            name = event_payload.command_ref
            (command_name, payload) = betterproto.which_one_of(
                event_payload, "CommandVariant"
            )

            if command_name != "stdout" and command_name != "profile":
                self.status_dict[name] = Status(command_name)
                if command_name == "started":
                    self.processed_started()

                if command_name == "finished":
                    payload = cast(CommandFinished, payload)
                    self.process_finished(payload.outputs.exit_code)

            # we are processing stdout of a command
            else:
                if self.progress and self.print_stdout:
                    self.progress.print(payload.output)

    def processed_started(self):
        self.total_executing += 1
        self.total_run += 1

    def process_finished(self, status_code: int):
        self.total_executing -= 1
        if status_code == 0:
            self.total_passed += 1

        else:
            pass

    def reset(self):
        self.is_done = False
