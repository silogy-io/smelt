from typing import List, Dict, Literal
from enum import Enum
from otl.interfaces.runtime import RuntimeRequirements
from otl.interfaces.target import OtlTargetType, Target
from dataclasses import dataclass, asdict

CommandRef = str


@dataclass
class Command:
    name: str
    target_type: OtlTargetType
    script: List[str]
    depdenencies: CommandRef
    outputs: List[str]
    runtime: RuntimeRequirements
    runtime_env: Dict[str, str]

    @classmethod
    def from_target(cls, target: Target):
        name = target.name
        target_type = target.rule_type().value
        script = target.gen_script()
        runtime = target.runtime_requirements()
        runtime_env = target.runtime_env_vars()
        dependencies = target.dependencies()

        outputs = list(map(lambda path: str(path),
                           target.get_outputs().values()))

        return cls(
            name=name,
            target_type=target_type,
            script=script,
            runtime=runtime,
            runtime_env=runtime_env,
            depdenencies=dependencies,
            outputs=outputs,
        )


class CStatus(Enum):
    PASS = "pass"
    FAIL = "failed"
    SKIPPED = "skipped"


CStatusStr = Literal[CStatus.PASS.value,
                     CStatus.FAIL.value, CStatus.SKIPPED.value]


@dataclass
class CResult:
    name: str
    status: CStatusStr
