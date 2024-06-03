from dataclasses import dataclass
from typing import cast
import betterproto
from pysmelt.smelt_telemetry.data import Event, SmeltError, SmeltErrorType


class ClientErr(RuntimeError):
    """
    Error that gets thrown when we  receive a client error message
    """

    pass


class SmeltErrEx(RuntimeError):
    """
    Error that gets thrown when we receive an internal SMELT error
    """

    pass


@dataclass
class SmeltErrorHandler:
    """
    Simple subscriber that tells us if we're done executing
    """

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "error":
            error = cast(SmeltError, event_payload)
            if error.sig == SmeltErrorType.CLIENT_ERROR:
                raise ClientErr(error.error_payload)
            if error.sig == SmeltErrorType.INTERNAL_ERROR:
                print("hello")
                raise SmeltErrEx(error.error_payload)
        else:
            pass
