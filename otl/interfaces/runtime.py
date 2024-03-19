from dataclasses import dataclass
from typing import Literal
from enum import Enum


@dataclass
class RuntimeRequirements:
    num_cpus: int
    # This number is in MB
    max_memory_mb: int
    # timeout in seconds
    timeout: int

    @classmethod
    def default(cls):
        return cls(num_cpus=1, max_memory_mb=1024, timeout=600)



