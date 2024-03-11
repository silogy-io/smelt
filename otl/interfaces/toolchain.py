from dataclasses import dataclass
from typing import Union, Callable


@dataclass(frozen=True)
class ToolchainProvider:
    path: str
