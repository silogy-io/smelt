

from dataclasses import dataclass, field
from otl.interfaces import Target, OtlPath
from typing import List, Dict


@dataclass
class passthrough_bash(Target):
    """
    Simple target for embedding raw bash commands in Otl 

    Environment variables avaible are: 
        * ${GIT_ROOT}: the git root of the current git workspace 
        * ${OTL_ROOT}: the root of the otl-workspace -- by default, this will be ${GIT_ROOT}/otl
        * ${TARGET_ROOT}: the working space of 
    """
    script: List[str] = field(default_factory=list)
    outputs: Dict[str, str] = field(default_factory=dict)

    def gen_script(self) -> List[str]:
        return self.script

    def get_outputs(self) -> Dict[str, OtlPath]:
        return {out_name: OtlPath.abs_path(out_path) for out_name, out_path in self.outputs.items()}
