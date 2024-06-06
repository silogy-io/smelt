from typing import Protocol

from pysmelt.proto.smelt_telemetry import Event


class SmeltSub(Protocol):
    def process_message(self, message: Event) -> None: ...
