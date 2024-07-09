from dataclasses import dataclass
from typing import Dict, Any, Optional


@dataclass
class RuntimeRequirements:
    num_cpus: int
    # This number is in MB
    max_memory_mb: int
    # timeout in seconds
    timeout: int

    @classmethod
    def default(
        cls,
    ):

        return cls(
            num_cpus=1,
            max_memory_mb=1024,
            timeout=600,
        )

    @classmethod
    def from_dict(cls, indict: Dict[str, Any]):
        num_cpus = indict["num_cpus"]
        max_memory_mb = indict["max_memory_mb"]
        timeout = indict["timeout"]

        return cls(num_cpus=num_cpus, max_memory_mb=max_memory_mb, timeout=timeout)
