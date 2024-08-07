from typing import List, Literal, Dict, Any, Optional, Tuple
from enum import Enum
from pysmelt.interfaces.runtime import RuntimeRequirements
from dataclasses import dataclass, asdict


from pysmelt.rc import SmeltRcHolder

CommandRef = str

# TODO: make this automatic from the targettype enum
CommandType = Literal["test", "stimulus", "build", "rebuild", "rerun"]


@dataclass
class Command:
    """
    The simplest unit of compute in smelt -- commands are the nodes that are scheduled and executed by the runtime

    Functionally, Command is a simple wrapper around a `bash` script
    """

    name: str
    target_type: CommandType
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
    working_dir: str
    on_failure: Optional[CommandRef] = None

    @classmethod
    def from_dict(cls, data: Dict[str, Any]):
        name = data["name"]
        target_type = data["target_type"]
        script = data["script"]
        dependencies = data["dependencies"] if "dependencies" in data else []
        dependent_files = data["dependent_files"] if "dependent_files" in data else []
        outputs = data["outputs"] if "outputs" in data else []
        working_dir = (
            data["working_dir"]
            if "working_dir" in data
            else SmeltRcHolder.current_smelt_root()
        )

        runtime = RuntimeRequirements.from_dict(data["runtime"])

        return cls(
            name=name,
            target_type=target_type,
            script=script,
            dependent_files=dependent_files,
            dependencies=dependencies,
            outputs=outputs,
            runtime=runtime,
            working_dir=working_dir,
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
