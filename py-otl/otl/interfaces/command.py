from typing import List, Literal, Dict, Any
from enum import Enum
from otl.interfaces.runtime import RuntimeRequirements
from otl.interfaces.target import OtlTargetType, Target
from dataclasses import dataclass, asdict

CommandRef = str


@dataclass
class Command:
    name: str
    # todo: this really needs to be an otl target type literal, but i am too annoyed at pyright now to figure it out
    target_type: str
    script: List[str]
    dependencies: List[CommandRef]
    outputs: List[str]
    runtime: RuntimeRequirements

    @classmethod
    def from_target(cls, target: Target, default_root: str):
        name = target.name
        target_type = target.rule_type().value
        script = target.gen_script()
        runtime = target.runtime_requirements()
        dependencies = target.dependencies()
        default_env = target.required_runtime_env_vars(default_root)
        runtime.env.update(default_env)

        outputs = list(map(lambda path: str(path), target.get_outputs().values()))

        return cls(
            name=name,
            target_type=target_type,
            script=script,
            runtime=runtime,
            dependencies=dependencies,
            outputs=outputs,
        )

    @classmethod
    def from_dict(cls, data: Dict[str, Any]):
        name = data["name"]
        target_type = data["target_type"]
        script = data["script"]
        dependencies = data["dependencies"]
        outputs = data["outputs"]
        runtime = RuntimeRequirements.from_dict(data["runtime"])

        return cls(
            name=name,
            target_type=target_type,
            script=script,
            dependencies=dependencies,
            outputs=outputs,
            runtime=runtime,
        )

    def to_dict(self) -> Dict[str, Any]:
        rv = asdict(self)

        return rv


class CStatus(Enum):
    PASS = "pass"
    FAIL = "failed"
    SKIPPED = "skipped"


CStatusStr = Literal[CStatus.PASS, CStatus.FAIL, CStatus.SKIPPED]  # ignore


@dataclass
class CResult:
    name: str
    status: CStatusStr