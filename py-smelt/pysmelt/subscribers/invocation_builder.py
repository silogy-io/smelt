from dataclasses import dataclass, field
from datetime import datetime
from typing import Dict, List, Optional, Tuple, cast
from pysmelt.output import smelt_console
import betterproto
from pysmelt.interfaces.analysis import (
    most_recent_invoke_path,
    most_recent_junit_path,
    read_log_from_result,
)
from pysmelt.interfaces.target import SmeltTargetType
from pysmelt.proto.smelt_telemetry import (
    CommandEvent,
    CommandFinished,
    CommandStarted,
    Event,
    ExecutionStart,
    InvokeEvent,
)
from pysmelt.proto.executed_tests import Invocation, TestResult


from junitparser.junitparser import (
    TestCase,
    TestSuite,
    JUnitXml,
    Skipped,
    Error,
    Failure,
)


@dataclass
class InvocationBuilder:
    """
    Subscriber to build an Invocation and JUnitXml object, that tracks outputs and other artifacts of tests


    """

    invoke_id: Optional[str] = None
    start: Optional[datetime] = None
    rundate: Optional[datetime] = None
    user: Optional[str] = None
    repo: Optional[str] = None
    branch: Optional[str] = None
    hostname: Optional[str] = None
    smelt_root: Optional[str] = None
    tests: List[Tuple[TestResult, str, datetime]] = field(default_factory=list)
    test_start: Dict[str, datetime] = field(default_factory=dict)

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "command":
            event_payload = cast(CommandEvent, event_payload)
            command_name = event_payload.command_ref
            (command_variant, command_payload) = betterproto.which_one_of(
                event_payload, "CommandVariant"
            )

            if command_variant == "started":
                command_payload = cast(CommandStarted, command_payload)
                self.test_start[command_name] = message.time

            if command_variant == "finished":
                command_payload = cast(CommandFinished, command_payload)

                self.tests.append(
                    (
                        TestResult(
                            test_name=command_name, outputs=command_payload.outputs
                        ),
                        command_payload.command_type,
                        message.time,
                    )
                )

            else:
                pass
        elif variant == "invoke":
            event_payload = cast(InvokeEvent, event_payload)
            (invoke_variant, invoke_payload) = betterproto.which_one_of(
                event_payload, "InvokeVariant"
            )

            if invoke_variant == "start":
                self.start = message.time
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

        results = [val[0] for val in self.tests]
        return Invocation(
            invoke_id=self.invoke_id,
            rundate=self.rundate,
            user=self.user,
            repo=self.repo,
            branch=self.branch,
            hostname=self.hostname,
            smelt_root=self.smelt_root,
            executed_tests=results,
        )

    def create_junit(self) -> JUnitXml:
        assert self.invoke_id, "invoke_id is required"
        assert self.rundate, "rundate is required"
        assert self.start, "start is required"
        assert self.smelt_root, "smelt_root is required"

        suite = TestSuite(name=self.invoke_id)
        if self.hostname:
            suite.hostname = self.hostname
        suite_duration = (self.rundate - self.start).total_seconds()
        suite.time = suite_duration

        for test in self.tests:
            tr, ttype, endtime = test
            duration = (endtime - self.test_start[tr.test_name]).total_seconds()
            case = TestCase(tr.test_name, ttype, duration)  # params are optional
            log = read_log_from_result(tr)
            if log:
                case.system_out = log
            if tr.outputs.exit_code != 0:
                case.result = [Failure(f"failed with exit code {tr.outputs.exit_code}")]
            suite.add_testcase(case)
        rv = JUnitXml()
        rv.add_testsuite(suite)
        return rv

    def write_invocation_and_junit(self):
        try:
            inv_obj = self.create_invocation_object().SerializeToString()
            with open(most_recent_invoke_path().to_abs_path(), "wb") as f:
                f.write(inv_obj)
            xml = self.create_junit()
            xml.write(most_recent_junit_path().to_abs_path(), pretty=True)
        except Exception as e:
            smelt_console.print("[yellow] Failed to create junit xml")
