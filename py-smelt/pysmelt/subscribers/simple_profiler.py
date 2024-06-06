from collections import defaultdict
from dataclasses import dataclass, field
from typing import DefaultDict, List, cast
import betterproto
from pysmelt.proto.smelt_telemetry import (
    CommandProfile,
    CommandEvent,
    Event,
)


@dataclass
class ProfileWatcher:
    """
    Simple subscriber that prints the stdout for a single command
    """

    profile_events: DefaultDict[str, List[CommandProfile]] = field(
        default_factory=lambda: defaultdict(list)
    )

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "command":
            event_payload = cast(CommandEvent, event_payload)
            (command_name, command_payload) = betterproto.which_one_of(
                event_payload, "CommandVariant"
            )
            if command_name == "profile":
                profile = cast(CommandProfile, command_payload)

                self.profile_events[event_payload.command_ref].append(profile)
            else:
                pass
        else:
            pass
