from pydantic.dataclasses import dataclass
from typing import ClassVar, Dict, Optional, Union
from pathlib import Path
from pysmelt.path_utils import get_git_root
import toml
import os
import inspect

from pprint import pprint
from dataclasses import replace


@dataclass(frozen=True)
class SmeltRC:

    smelt_root: str
    """
    Path to smelt root -- is $GIT_ROOT by default
    """
    smelt_rules_dir: str
    """
    Directory that holds third party smelt rules
    """

    jobs: int
    """
    Number of job slots that can be used
    """

    @classmethod
    def default(cls):
        default_jobs = 8
        try:
            smelt_root = get_git_root()
        except:
            smelt_root = os.getcwd()

        return cls(
            smelt_root=smelt_root,
            smelt_rules_dir="smelt_rules",
            jobs=default_jobs,
        )

    @classmethod
    def try_load(cls):
        git_root = get_git_root()
        rc_path = Path(f"{git_root}/.smeltrc")

        if not rc_path.exists():
            return SmeltRC.default()

        stream = rc_path.read_text()

        try:
            rc_content = toml.loads(stream)
            return cls(
                smelt_root=rc_content["smelt_root"],
                smelt_rules_dir=rc_content["smelt_rules_dir"],
                jobs=rc_content["jobs"],
            )
        except toml.TomlDecodeError as exc:
            print(exc)
            raise RuntimeError(exc)

    @staticmethod
    def init_rc():
        default = SmeltRC.default()
        git_root = get_git_root()
        rc_path = Path(f"{git_root}/.smeltrc")
        with open(rc_path, "w") as outfile:
            toml.dump(default.__dict__, outfile)
        smelt_rules_dir = Path(f"{git_root}/{default.smelt_rules_dir}").mkdir(
            exist_ok=True
        )
        pprint(f"Initialized .smeltrc at {rc_path}")

    @property
    def abs_rules_dir(self) -> Path:
        git_root = get_git_root()
        return Path(f"{git_root}/{self.smelt_rules_dir}")


class SmeltRcHolder:
    _current_rc: ClassVar[SmeltRC] = SmeltRC.default()

    @staticmethod
    def current_rc() -> SmeltRC:
        return SmeltRcHolder._current_rc

    @staticmethod
    def set_jobs(jobs: int):

        SmeltRcHolder._current_rc = replace(SmeltRcHolder._current_rc, jobs=jobs)

    @staticmethod
    def current_smelt_root() -> str:
        return SmeltRcHolder.current_rc().smelt_root
