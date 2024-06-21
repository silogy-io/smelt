from datetime import datetime
import enum
from typing import Dict, List, Optional, Tuple
from rich.table import Table
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
    finished_list: List[Tuple[CommandFinished, str, datetime]] = field(
        default_factory=list
    )
    start_time: Dict[str, datetime] = field(default_factory=dict)

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

        table = Table(show_header=True, header_style="bold magenta")
        table.add_column("Command Name")
        table.add_column("Exit Code")
        table.add_column("Execution Time")

        new_finished_list = [
            (obj, command_name, end_time - self.start_time[command_name])
            for obj, command_name, end_time in self.finished_list
        ]
        # toggle this if we want to show more or less
        topn = 10
        for obj, command_name, execution_time in sorted(
            new_finished_list, key=lambda x: x[2], reverse=True
        )[:topn]:

            total_seconds = execution_time.total_seconds()
            minutes, seconds = divmod(total_seconds, 60)
            int_seconds, milliseconds = divmod(seconds, 1)

            time_str = ""
            if minutes > 0:
                time_str += f"{int(minutes)}m "
            if int_seconds > 0:
                time_str += f"{seconds:.2f}s"
            elif milliseconds > 0:
                time_str += f"{milliseconds:.2f}ms "

            smelt_console.log(execution_time.seconds)
            table.add_row(command_name, str(obj.outputs.exit_code), time_str)

        # Print the table
        smelt_console.log(table)

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
                    self.processed_started(name, message.time)

                if command_name == "finished":
                    payload = cast(CommandFinished, payload)
                    self.process_finished(payload, name, message.time)

            # we are processing stdout of a command
            else:
                if self.progress and self.print_stdout:
                    self.progress.print(payload.output)

    def processed_started(self, name: str, time: datetime):
        self.total_executing += 1
        self.total_run += 1
        self.start_time[name] = time

    def process_finished(self, obj: CommandFinished, command_name: str, time: datetime):
        self.total_executing -= 1
        if obj.outputs.exit_code == 0:
            self.total_passed += 1
        else:
            pass
        self.finished_list.append((obj, command_name, time))

    def reset(self):
        self.is_done = False
