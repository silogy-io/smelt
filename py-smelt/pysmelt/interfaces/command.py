from typing import List, Literal, Dict, Any
from enum import Enum
from pysmelt.interfaces.runtime import RuntimeRequirements
from pysmelt.interfaces.target import SmeltTargetType, Target
from dataclasses import dataclass, asdict

CommandRef = str

CommandLiterals = Literal["test", "stimulus", "build"]


@dataclass
class Command:
    """
    The simplest unit of compute in smelt -- commands are the nodes that are scheduled and executed by the runtime

    Functionally, Command is a simple wrapper around a `bash` script
    """

    name: str
    target_type: Literal["test", "stimulus", "build"]
    script: List[str]
    """
    A list of bash commands that will be executed in sequence
    """
    dependencies: List[CommandRef]
    dependent_files: List[str]

    """
    Paths to the files that are expected to be created by this command 

    Anything here will be treated as an "artifact" 
    """
    outputs: List[str]
    runtime: RuntimeRequirements

    @classmethod
    def from_target(cls, target: Target, default_root: str):
        name = target.name
        target_type = target.rule_type().value
        script = target.gen_script()
        runtime = target.runtime_requirements()
        dependencies = target.get_dependencies()
        dependent_files = target.get_dependent_files()

        outputs = list(map(lambda path: str(path), target.get_outputs().values()))

        return cls(
            name=name,
            target_type=target_type,
            script=script,
            runtime=runtime,
            dependencies=dependencies,
            dependent_files=dependent_files,
            outputs=outputs,
        )

    @classmethod
    def from_dict(cls, data: Dict[str, Any]):
        name = data["name"]
        target_type = data["target_type"]
        script = data["script"]
        dependencies = data["dependencies"] if "dependencies" in data else []
        dependent_files = data["dependent_files"] if "dependent_files" in data else []
        outputs = data["outputs"] if "outputs" in data else []
        runtime = RuntimeRequirements.from_dict(data["runtime"])

        return cls(
            name=name,
            target_type=target_type,
            script=script,
            dependent_files=dependent_files,
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
