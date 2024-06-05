from collections import defaultdict
from dataclasses import dataclass, field
from typing import Callable, Dict, Optional, cast
import betterproto
from pysmelt.proto.smelt_telemetry import (
    CommandEvent,
    CommandFinished,
    CommandStdout,
    Event,
)


StdoutSink = Callable[[str], None]


@dataclass
class StdoutPrinter:
    """
    Simple subscriber that prints the stdout for a single command
    """

    command_ref: str
    sink: StdoutSink

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "command":
            event_payload = cast(CommandEvent, event_payload)
            (command_name, command_payload) = betterproto.which_one_of(
                event_payload, "CommandVariant"
            )
            if command_name == "stdout":
                command_payload = cast(CommandStdout, command_payload)
                self.sink(command_payload.output)

            else:
                pass
        else:
            pass
