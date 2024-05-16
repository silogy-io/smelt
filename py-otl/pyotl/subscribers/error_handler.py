from dataclasses import dataclass
from typing import cast
import betterproto
from pyotl.otl_telemetry.data import Event, OtlError, OtlErrorType


class ClientErr(RuntimeError):
    """
    Error that gets thrown when we  receive a client error message
    """

    pass


class OtlErrEx(RuntimeError):
    """
    Error that gets thrown when we receive an internal OTL error
    """

    pass


@dataclass
class OtlErrorHandler:
    """
    Simple subscriber that tells us if we're done executing
    """

    def process_message(self, message: Event):
        (variant, event_payload) = betterproto.which_one_of(message, "et")
        if variant == "error":
            error = cast(OtlError, event_payload)
            if error.sig == OtlErrorType.CLIENT_ERROR:
                raise ClientErr(error.error_payload)
            if error.sig == OtlErrorType.INTERNAL_ERROR:
                print("hello")
                raise OtlErrEx(error.error_payload)
        else:
            pass
