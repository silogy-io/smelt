from dataclasses import dataclass, field
from typing import Dict, cast
import betterproto
from pysmelt.proto.smelt_telemetry import CommandEvent, CommandFinished, Event
from pysmelt.proto.executed_tests import Invocation


@dataclass
class RetcodeTracker:
    """
    Simple subscriber that maps commands to their return codes
    """

    retcode_dict: Dict[str, int] = field(default_factory=dict)

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "command":
            event_payload = cast(CommandEvent, event_payload)
            (command_name, command_payload) = betterproto.which_one_of(
                event_payload, "CommandVariant"
            )

            if command_name == "finished":
                command_payload = cast(CommandFinished, command_payload)
                self.retcode_dict[event_payload.command_ref] = (
                    command_payload.outputs.exit_code
                )
            else:
                pass
        else:
            pass

    def total_executed(self) -> int:
        return len(self.retcode_dict.items())

    def total_passed(self) -> int:
        return sum(1 for rc in self.retcode_dict.values() if rc == 0)

    def total_failed(self) -> int:
        return sum(1 for rc in self.retcode_dict.values() if rc != 0)

    def reset(self):
        self.retcode_dict = {}
