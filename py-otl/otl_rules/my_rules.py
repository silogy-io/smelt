from dataclasses import dataclass
from pyotl.interfaces import Target, OtlPath
from typing import List, Dict


@dataclass
class run_spi2(Target):
    """
    sanity test -- will move this to examples, eventually
    """

    seed: int

    def gen_script(self) -> List[str]:
        return ['echo "hello world"']

    def get_outputs(self) -> Dict[str, OtlPath]:
        return {"log": OtlPath.abs_path(f"{self.name}.log")}
