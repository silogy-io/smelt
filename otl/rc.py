
from pydantic.dataclasses import dataclass
from typing import Dict
from pathlib import Path
from otl.path_utils import get_git_root
import toml

from pprint import pprint


@dataclass
class OtlRC:
    otl_default_root: str
    otl_rules_dir: str

    @classmethod
    def default(cls):
        return cls(otl_default_root="otl-out", otl_rules_dir="otl_rules")

    @classmethod
    def try_load(cls):
        git_root = get_git_root()
        rc_path = Path("{git_root}/.otlrc")
        if not rc_path.exists():
            print(
                "WARNING: otlrc is unitialized! execute `otl-cli init` to create all the expected scaffolding")
            return OtlRC.default()

        stream = rc_path.read_text()

        try:
            rc_content = toml.loads(stream)
            return cls(otl_default_root=Path(rc_content['otl_default_root']), otl_rules_dir=Path(rc_content['otl_rules_dir']))
        except toml.TomlDecodeError as exc:
            print(exc)
            return None

    @staticmethod
    def init_rc():
        default = OtlRC.default()
        git_root = get_git_root()
        rc_path = Path(f"{git_root}/.otlrc")
        with open(rc_path, 'w') as outfile:
            toml.dump(default.__dict__, outfile)
        otl_rules_dir = Path(
            f"{git_root}/{default.otl_rules_dir}").mkdir(exist_ok=True)
        pprint(f"Initialized .otlrc at {rc_path}")

    @property
    def abs_rules_dir(self) -> Path:
        git_root = get_git_root()
        return Path(f"{git_root}/{self.otl_rules_dir}")
