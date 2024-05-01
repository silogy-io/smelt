import enum
from typing import Dict, Optional
from rich.console import Group
from rich.tree import Tree
import rich
from pyotl.output import otl_console
from dataclasses import dataclass, field
from pyotl.otl_telemetry.data import CommandEvent, CommandFinished, Event
import betterproto
from rich.progress import (
    Progress,
    RenderableColumn,
    SpinnerColumn,
    Task,
    TaskID,
    TimeElapsedColumn,
)
import time


class Status(enum.Enum):
    scheduled = "scheduled"
    started = "started"
    finished = "finished"
    cancelled = "cancelled"


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
            "Executing otl...", fields={"status": self.status_dict}
        )

        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.progress:
            self.progress.stop()
            self.progress = None
        otl_console.log(
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
            self.status_dict[name] = Status(command_name)
            if command_name == "started":
                self.processed_started()

            if command_name == "finished":
                payload: CommandFinished
                self.process_finished(payload.out.status_code)

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
