from typing import List, Literal, Dict, Any, Optional, Tuple
from enum import Enum
from pysmelt.interfaces.paths import SmeltPath, TempTarget
from pysmelt.interfaces.runtime import RuntimeRequirements
from pysmelt.interfaces.target import CGVar, SmeltTargetType, Target
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
    def from_target(
        cls, target: Target, working_dir: str
    ) -> Tuple["Command", Optional["Command"], Optional["Command"]]:
        name = target.name
        target_type = target.rule_type().value
        script = target.gen_script()
        runtime = target.runtime_requirements()
        dependencies = target.get_dependencies()
        dependent_files = target.get_dependent_files()

        rerun_script = target.gen_rerun_script()
        rebuild_script = target.gen_rebuild_script()
        outputs = list(map(lambda path: str(path), target.get_outputs().values()))

        base_target = cls(
            name=name,
            target_type=target_type,
            script=script,
            runtime=runtime,
            dependencies=dependencies,
            dependent_files=dependent_files,
            outputs=outputs,
            working_dir=working_dir,
            on_failure=f"{name}@rerun" if rerun_script else None,
        )
        rebuild_target = None
        rerun_target = None

        if rebuild_script:
            rebuild_target = cls(
                name=f"{name}@rebuild",
                target_type=SmeltTargetType.Rebuild.value,
                script=rebuild_script,
                runtime=target.runtime_requirements(CGVar.rebuild),
                dependencies=dependencies,
                dependent_files=target.get_dependent_files(CGVar.rebuild),
                outputs=list(
                    map(
                        lambda path: str(path), target.get_outputs(CGVar.rerun).values()
                    )
                ),
                working_dir=working_dir,
            )

        if rerun_script:
            rerun_target = cls(
                name=f"{name}@rerun",
                target_type=SmeltTargetType.Rerun.value,
                script=rerun_script,
                runtime=target.runtime_requirements(CGVar.rerun),
                dependencies=dependencies,
                dependent_files=target.get_dependent_files(CGVar.rerun),
                outputs=list(
                    map(
                        lambda path: str(path), target.get_outputs(CGVar.rerun).values()
                    )
                ),
                working_dir=working_dir,
            )
        return base_target, rebuild_target, rerun_target

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
