from typing import List, Dict
from otl.interfaces.action_status import RuntimeRequirements
from otl.interfaces.target import OtlTargetType


class Command:
    name: str
    target_type: OtlTargetType
    script: List[str]
    runtime: RuntimeRequirements
    runtime_env: Dict[str, str]
    depdenencies: List['Command']
