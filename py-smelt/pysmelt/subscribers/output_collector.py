import collections
from datetime import datetime, timezone, timedelta
import enum
from typing import Dict, List, Optional, Tuple

import statistics

from rich.text import Text
from rich.columns import Columns

from pysmelt.rc import SmeltRcHolder
from rich.table import Table
from typing_extensions import cast
from rich.console import Group, RenderableType
from rich.tree import Tree
import rich
from pysmelt.interfaces.target import SmeltTargetType, Target
from pysmelt.output import smelt_console
from dataclasses import dataclass, field
from pysmelt.proto.smelt_telemetry import (
    CommandEvent,
    CommandFinished,
    CommandProfile,
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


def format_time(total_seconds: float, rich_conformant: bool = False) -> str:
    hours, minutes = divmod(total_seconds, 3600)
    minutes, seconds = divmod(total_seconds, 60)
    time_str = ""
    if rich_conformant:
        return "{:01}:{:02}:{:02}".format(int(hours), int(minutes), int(seconds))

    if hours > 0:
        time_str += f"{int(hours)}h "
    if minutes > 0:
        time_str += f"{int(minutes)}m "
    if seconds > 0:
        time_str += f"{seconds:.3f}s "

    return time_str.strip()


def stringify_mem(num_bytes: float):
    """
    this function will convert bytes to MB.... GB... etc
    """
    for x in ["B", "KB", "MB", "GB", "TB"]:
        if num_bytes < 1024.0:
            return "%3.1f %s" % (num_bytes, x)
        num_bytes /= 1024.0
    return ""


class RenderableTree(RenderableColumn):
    """Tasks that are running"""

    def render(self, task: Task):
        """Show data completed."""
        status_dict: Dict[str, Status] = task.fields["fields"]["status"]
        profile_dict: Dict[str, CommandProfile] = task.fields["fields"]["profile"]
        timedict: Dict[str, datetime] = task.fields["fields"]["start"]

        just_size = 11
        outer_just = 59
        inner_just = 55

        def get_total_tests():
            finished_keys = [k for k, v in status_dict.items() if v == Status.finished]
            num_finished_keys = len(finished_keys)
            total_keys = len(status_dict)
            return Text(
                f"{num_finished_keys}/{total_keys} executed".ljust(outer_just),
                style="bold",
                justify="left",
            )

        total_tests = get_total_tests()

        def get_elapsed():
            elapsed = task.finished_time if task.finished else task.elapsed
            if elapsed is None:
                return Text("-:--:--", style="progress.elapsed")
            delta = timedelta(seconds=max(0, int(elapsed)))
            return Text(str(delta), style="progress.elapsed")

        elapsed = get_elapsed()

        def get_agg(task: Task):
            profile_dict: Dict[str, CommandProfile] = task.fields["fields"]["profile"]
            status_dict: Dict[str, Status] = task.fields["fields"]["status"]
            max_load_and_mem: List[float] = task.fields["fields"]["max"]

            mem = 0.0
            load = 0.0
            for command, status in status_dict.items():
                profile = profile_dict[command] if command in profile_dict else None
                if status == Status.started:
                    if profile:

                        mem += profile.memory_used
                        load += profile.cpu_load
            if mem > max_load_and_mem[0]:
                max_load_and_mem[0] = mem
            if load > max_load_and_mem[1]:
                max_load_and_mem[1] = load
            memstr = stringify_mem(mem)

            return Text(f"{memstr.ljust(just_size)} " + f"{load:.2f}".ljust(just_size))

        root = Tree(
            Columns(("Total".ljust(outer_just), get_agg(task), elapsed), align="left")
        )
        for command, status in status_dict.items():
            profile = profile_dict[command] if command in profile_dict else None
            if status == Status.started:
                execution_time = datetime.now(timezone.utc) - timedict[command]
                total_seconds = execution_time.total_seconds()
                timestr = format_time(total_seconds, rich_conformant=True)
                name = Text(
                    f"Running {command}".ljust(inner_just),
                )

                time = Text(f"{timestr}")

                if profile:
                    mem = stringify_mem(profile.memory_used)
                    cpu_load = f"{profile.cpu_load:.2f}"
                    sub1 = root.add(
                        Columns(
                            [
                                name,
                                Text(f"{mem}".ljust(just_size)),
                                Text(f"{cpu_load}".ljust(just_size)),
                                time,
                            ],
                            align="left",
                        )
                    )

                else:
                    sub1 = root.add(Columns([name, "".ljust(just_size * 2 + 1), time]))

        topper = Columns(
            (
                total_tests,
                "memory".ljust(just_size),
                "cpu load".ljust(just_size),
                "elapsed".ljust(just_size),
            )
        )
        return Columns((topper, root), column_first=True)


def get_logs_from_command_name(command_name: str, num_lines_tail: int = 3) -> str:
    root = SmeltRcHolder.current_rc().smelt_root
    try:
        with open(f"{root}/smelt-out/{command_name}/command.out", "r") as f:
            lines = collections.deque(f, num_lines_tail)
        return "\n".join(lines)
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
    total_tests_passed: int = 0
    total_tests_failed: int = 0
    total_scheduled: int = 0
    status_dict: Dict[str, Status] = field(default_factory=dict)
    profile_dict: Dict[str, CommandProfile] = field(default_factory=dict)
    progress: Optional[Progress] = None
    task: Optional[TaskID] = None
    finished_list: List[Tuple[CommandFinished, str, datetime]] = field(
        default_factory=list
    )
    max_load_and_mem: List[float] = field(default_factory=list)
    skipped_list: List[str] = field(default_factory=list)

    start_time: Dict[str, datetime] = field(default_factory=dict)

    def __enter__(self):

        self.progress = Progress(
            RenderableTree(),
        )
        self.max_load_and_mem = [0, 0]

        self.progress.start()
        self.task = self.progress.add_task(
            "Executing smelt...",
            fields={
                "status": self.status_dict,
                "profile": self.profile_dict,
                "max": self.max_load_and_mem,
                "start": self.start_time,
            },
        )

        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.progress:
            self.progress.stop()
            self.progress = None

        smelt_console.log(
            f"Executed {self.total_run} commands, {self.total_passed} commands passed"
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
            time_str = format_time(total_seconds)

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
        smelt_console.print(f"[green] {self.total_tests_passed} tests passed ")
        if len(self.skipped_list) != 0:
            smelt_console.print(f"[red] {len(self.skipped_list)} commands skipped")
        if failed != 0:
            smelt_console.print(
                f"[red] {self.total_tests_failed} tests failed, {failed} commands failed"
            )
        execution_times = [
            x[2].total_seconds()
            for x in new_finished_list
            if x[0].command_type == SmeltTargetType.Test.value
        ]

        try:
            average_time = sum(execution_times) / len(execution_times)
            stdev_time = statistics.stdev(execution_times)
            smelt_console.print(
                f"avg test duration {average_time:.2f}s, stddev {stdev_time:.3f}s"
            )
        except Exception as _:
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
                if command_name == "scheduled":
                    self.total_scheduled += 1
                if command_name == "started":
                    self.processed_started(name, message.time)

                if command_name == "finished":
                    payload = cast(CommandFinished, payload)
                    self.process_finished(payload, name, message.time)

                if command_name == "skipped":
                    self.process_skipped(name)
            if command_name == "profile":
                payload = cast(CommandProfile, payload)
                self.profile_dict[name] = payload

            # we are processing stdout of a command
            else:
                if self.progress and self.print_stdout:
                    payload = cast(CommandStdout, payload)
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
            if obj.command_type == SmeltTargetType.Test.value:
                self.total_tests_passed += 1
        elif (
            obj.outputs.exit_code != 0
            and obj.command_type == SmeltTargetType.Test.value
        ):
            self.total_tests_failed += 1
        else:
            pass
        self.finished_list.append((obj, command_name, time))

    def reset(self):
        self.is_done = False
