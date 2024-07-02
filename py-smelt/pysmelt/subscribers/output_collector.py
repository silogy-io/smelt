import collections
from datetime import datetime
import enum
from typing import Dict, List, Optional, Tuple
from pysmelt.rc import SmeltRcHolder
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
    skipped = "skipped"


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


def get_logs_from_command_name(command_name: str, num_lines_tail: int = 3) -> str:
    root = SmeltRcHolder.current_rc().smelt_root
    try:
        log = open(f"{root}/smelt-out/{command_name}/command.out", "r")
        return "\n".join(collections.deque(log, num_lines_tail))
    except Exception as e:
        return f"Could not read {command_name}'s log"


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
    skipped_list: List[str] = field(default_factory=list)

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
        table.add_column("Status")
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

            table.add_row(
                command_name,
                (
                    "PASSED"
                    if obj.outputs.exit_code == 0
                    else f"FAILED, code: {obj.outputs.exit_code}"
                ),
                time_str,
            )
        if len(new_finished_list) > topn:
            unseen = len(new_finished_list) - topn
            table.add_row(f"and {unseen} other commands...", "")
        smelt_console.log(table)
        fail_table = Table(
            show_header=True, title="Failed commands", header_style="bold magenta"
        )
        fail_table.add_column("Command Name")
        fail_table.add_column("log tail")

        one_failed = False
        for obj, command_name, execution_time in filter(
            lambda x: x[0].outputs.exit_code != 0,
            sorted(new_finished_list, key=lambda x: x[2], reverse=True),
        ):
            one_failed = True
            logs = get_logs_from_command_name(command_name)
            fail_table.add_row(command_name, logs)
        if one_failed:
            smelt_console.log(fail_table)

        failed = self.total_run - self.total_passed
        smelt_console.print(f"[green] {self.total_passed} commands passed ")
        if len(self.skipped_list) != 0:
            smelt_console.print(f"[red] {len(self.skipped_list)} commands skipped")
        if failed != 0:
            smelt_console.print(f"[red] {failed} commands failed")

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

                if command_name == "skipped":
                    self.process_skipped(name)

            # we are processing stdout of a command
            else:
                if self.progress and self.print_stdout:
                    self.progress.print(payload.output)

    def processed_started(self, name: str, time: datetime):
        self.total_executing += 1
        self.total_run += 1
        self.start_time[name] = time

    def process_skipped(self, name: str):
        self.skipped_list.append(name)

    def process_finished(self, obj: CommandFinished, command_name: str, time: datetime):
        self.total_executing -= 1
        if obj.outputs.exit_code == 0:
            self.total_passed += 1
        else:
            pass
        self.finished_list.append((obj, command_name, time))

    def reset(self):
        self.is_done = False
