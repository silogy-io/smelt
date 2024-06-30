from dataclasses import dataclass, field
from datetime import datetime
from typing import Dict, List, Optional, cast
import betterproto
from pysmelt.interfaces.analysis import most_recent_invoke_path
from pysmelt.proto.smelt_telemetry import (
    CommandEvent,
    CommandFinished,
    Event,
    ExecutionStart,
    InvokeEvent,
)
from pysmelt.proto.executed_tests import Invocation, TestResult


@dataclass
class InvocationBuilder:
    """
    Simple subscriber that maps commands to their return codes
    """

    invoke_id: Optional[str] = None
    rundate: Optional[datetime] = None
    user: Optional[str] = None
    repo: Optional[str] = None
    branch: Optional[str] = None
    hostname: Optional[str] = None
    smelt_root: Optional[str] = None
    tests: List[TestResult] = field(default_factory=list)

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "command":
            event_payload = cast(CommandEvent, event_payload)
            command_name = event_payload.command_ref
            (command_variant, command_payload) = betterproto.which_one_of(
                event_payload, "CommandVariant"
            )

            if command_variant == "finished":
                command_payload = cast(CommandFinished, command_payload)

                self.tests.append(
                    TestResult(test_name=command_name, outputs=command_payload.outputs)
                )

            else:
                pass
        elif variant == "invoke":
            event_payload = cast(InvokeEvent, event_payload)
            (invoke_variant, invoke_payload) = betterproto.which_one_of(
                event_payload, "InvokeVariant"
            )

            if invoke_variant == "start":
                invoke_payload = cast(ExecutionStart, invoke_payload)
                self.branch = invoke_payload.git_branch
                self.repo = invoke_payload.git_repo
                self.hostname = invoke_payload.hostname
                self.smelt_root = invoke_payload.smelt_root
                self.invoke_id = message.trace_id
                self.user = invoke_payload.username
            if invoke_variant == "done":
                self.rundate = message.time

        else:
            pass

    def create_invocation_object(self) -> Invocation:
        assert self.invoke_id, "invoke_id is required"
        assert self.rundate, "rundate is required"
        # these 4 are optional
        # assert self.user, "user is required"
        # assert self.repo, "repo is required"
        # assert self.branch, "branch is required"
        # assert self.hostname, "hostname is required"
        assert self.smelt_root, "smelt_root is required"

        return Invocation(
            invoke_id=self.invoke_id,
            rundate=self.rundate,
            user=self.user,
            repo=self.repo,
            branch=self.branch,
            hostname=self.hostname,
            smelt_root=self.smelt_root,
            executed_tests=self.tests,
        )

    def write_invocation_to_fs(self):
        inv_obj = self.create_invocation_object().SerializeToString()
        open(most_recent_invoke_path().to_abs_path(), ("wb")).write(inv_obj)
