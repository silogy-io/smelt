from pydantic.dataclasses import dataclass
from typing import ClassVar, Dict, Optional
from pathlib import Path
from pyotl.path_utils import get_git_root
import toml

from pprint import pprint


@dataclass(frozen=True)
class OtlRC:
    otl_default_root: str
    otl_rules_dir: str
    jobs: int

    @classmethod
    def default(cls):
        default_jobs = 8
        return cls(
            otl_default_root="otl-out", otl_rules_dir="otl_rules", jobs=default_jobs
        )

    @classmethod
    def try_load(cls):
        git_root = get_git_root()
        rc_path = Path(f"{git_root}/.otlrc")

        if not rc_path.exists():
            return OtlRC.default()

        stream = rc_path.read_text()

        try:
            rc_content = toml.loads(stream)
            return cls(
                otl_default_root=rc_content["otl_default_root"],
                otl_rules_dir=rc_content["otl_rules_dir"],
                jobs=rc_content["jobs"],
            )
        except toml.TomlDecodeError as exc:
            print(exc)
            raise RuntimeError(exc)

    @staticmethod
    def init_rc():
        default = OtlRC.default()
        git_root = get_git_root()
        rc_path = Path(f"{git_root}/.otlrc")
        with open(rc_path, "w") as outfile:
            toml.dump(default.__dict__, outfile)
        otl_rules_dir = Path(f"{git_root}/{default.otl_rules_dir}").mkdir(exist_ok=True)
        pprint(f"Initialized .otlrc at {rc_path}")

    @property
    def abs_rules_dir(self) -> Path:
        git_root = get_git_root()
        return Path(f"{git_root}/{self.otl_rules_dir}")


class OtlRcHolder:
    current_rc: ClassVar[OtlRC] = OtlRC.default()
