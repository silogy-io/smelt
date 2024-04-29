from otl.otl import PySubscriber
from dataclasses import dataclass
import betterproto
from otl.otl_telemetry.data import Event, InvokeEvent


from typing import List, Protocol


class MessageProcessor(Protocol):
    def process_message(self, message: Event) -> None: ...


@dataclass
class PyBroker:
    """
    Simple message broker that listens for a message and forwards it to subscribers
    """

    listener: PySubscriber
    downstream: List[MessageProcessor]

    def update(self, blocking: bool = True) -> bool:
        message = None
        if blocking:
            message = self.listener.pop_message_blocking()
            event = Event.FromString(message)
        else:
            message = self.listener.nonblocking_pop()
            if message is None:
                return False
            event = Event.FromString(message)
        for listener in self.downstream:
            listener.process_message(event)
        return True
