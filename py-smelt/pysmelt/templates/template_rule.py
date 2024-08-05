_template_fn = f"""
from dataclasses import dataclass, field
from pysmelt.interfaces import Target, SmeltFilePath, SmeltTargetType, TargetRef
from typing import List, Dict


@dataclass
class your_rule(Target):
    deps: List[TargetRef] = field(default_factory=list)

    def gen_script(self) -> List[str]:
        raise NotImplementedError

    def get_dependencies(self) -> List[TargetRef]:
        raise NotImplementedError
        return self.deps
"""


def create_rule_target_from_template(filename: str):
    with open(filename, "w") as f:
        f.write(_template_fn)
