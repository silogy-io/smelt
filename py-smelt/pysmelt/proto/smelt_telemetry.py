# Generated by the protocol buffer compiler.  DO NOT EDIT!
# sources: data.proto
# plugin: python-betterproto
from dataclasses import dataclass
from datetime import datetime

import betterproto

from . import executed_tests


class SmeltErrorType(betterproto.Enum):
    # Client caused error
    CLIENT_ERROR = 0
    # Internal error -- anything that is thrown by the smelt runtime
    INTERNAL_ERROR = 1
    # Internal warning -- anything that the smelt runtime wants to broadcast back
    INTERNAL_WARN = 2


@dataclass
class Event(betterproto.Message):
    """Event flows from server -> client only"""

    time: datetime = betterproto.message_field(1)
    # A globally-unique ID (UUIDv4) of this trace. Required.
    trace_id: str = betterproto.string_field(2)
    command: "CommandEvent" = betterproto.message_field(15, group="et")
    invoke: "InvokeEvent" = betterproto.message_field(16, group="et")
    error: "SmeltError" = betterproto.message_field(17, group="et")


@dataclass
class CommandEvent(betterproto.Message):
    """CommandEvents covers activity happening on a per target basis"""

    # test def id this ref should be consistent for the same test being executed
    command_ref: str = betterproto.string_field(1)
    scheduled: "CommandScheduled" = betterproto.message_field(4, group="CommandVariant")
    started: "CommandStarted" = betterproto.message_field(5, group="CommandVariant")
    cancelled: "CommandCancelled" = betterproto.message_field(6, group="CommandVariant")
    finished: "CommandFinished" = betterproto.message_field(7, group="CommandVariant")
    stdout: "CommandStdout" = betterproto.message_field(8, group="CommandVariant")
    profile: "CommandProfile" = betterproto.message_field(9, group="CommandVariant")
    skipped: "CommandSkipped" = betterproto.message_field(10, group="CommandVariant")


@dataclass
class CommandScheduled(betterproto.Message):
    pass


@dataclass
class CommandStarted(betterproto.Message):
    pass


@dataclass
class CommandCancelled(betterproto.Message):
    pass


@dataclass
class CommandSkipped(betterproto.Message):
    pass


@dataclass
class CommandStdout(betterproto.Message):
    output: str = betterproto.string_field(1)


@dataclass
class CommandFinished(betterproto.Message):
    outputs: executed_tests.TestOutputs = betterproto.message_field(1)


@dataclass
class CommandProfile(betterproto.Message):
    # memory used by the command, in bytes
    memory_used: int = betterproto.uint64_field(1)
    cpu_load: float = betterproto.float_field(2)


@dataclass
class InvokeEvent(betterproto.Message):
    """InvokeEvent demarcates the start of a graph execution."""

    start: "ExecutionStart" = betterproto.message_field(5, group="InvokeVariant")
    done: "AllCommandsDone" = betterproto.message_field(6, group="InvokeVariant")
    set: "SetGraph" = betterproto.message_field(7, group="InvokeVariant")


@dataclass
class ExecutionStart(betterproto.Message):
    smelt_root: str = betterproto.string_field(1)
    username: str = betterproto.string_field(2)
    hostname: str = betterproto.string_field(3)
    git_hash: str = betterproto.string_field(4)
    git_repo: str = betterproto.string_field(5)
    git_branch: str = betterproto.string_field(6)


@dataclass
class AllCommandsDone(betterproto.Message):
    pass


@dataclass
class SetGraph(betterproto.Message):
    pass


@dataclass
class SmeltError(betterproto.Message):
    sig: "SmeltErrorType" = betterproto.enum_field(1)
    error_payload: str = betterproto.string_field(2)
