from dataclasses import dataclass
import betterproto
from pysmelt.proto.smelt_telemetry import Event


@dataclass
class IsDoneSubscriber:
    """
    Simple subscriber that tells us if we're done executing
    """

    is_done: bool = False

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "invoke":
            (invoke_name, invoke_payload) = betterproto.which_one_of(
                event_payload, "InvokeVariant"
            )

            if invoke_name == "done":
                self.is_done = True
            else:
                pass
        else:
            pass

    def reset(self):
        self.is_done = False
