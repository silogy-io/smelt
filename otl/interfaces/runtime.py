from dataclasses import dataclass
from typing import Dict


@dataclass
class RuntimeRequirements:
    num_cpus: int
    # This number is in MB
    max_memory_mb: int
    # timeout in seconds
    timeout: int
    env: Dict[str, str]

    @classmethod
    def default(cls, env: Dict[str, str]):
        return cls(num_cpus=1, max_memory_mb=1024, timeout=600, env=env)
