from typing import List, Dict
from multiprocessing import Pool
from pathlib import Path
import subprocess
import time
from functools import partial

from otl.rc import OtlRC
from otl.path_utils import get_git_root
from otl.interfaces.command import CResult, Command


def default_environment(rc: OtlRC, command: Command) -> Dict[str, str]:
    git_root = get_git_root()
    otl_root = f"{git_root}/{rc.otl_default_root}"
    target_root = f"{otl_root}/{command.name}"

    return dict(GIT_ROOT=git_root, OTL_ROOT=otl_root,
                TARGET_ROOT=target_root)


def create_scaffolding(rc: OtlRC, command: Command):
    env = default_environment(rc, command)
    working_dir = Path(env['TARGET_ROOT'])
    script_file = working_dir / "command.sh"
    working_dir.mkdir(parents=True, exist_ok=True)
    with script_file.open('w') as f:
        for env_name, env_val in env.items():
            f.write(f'export {env_name}={env_val}\n')
        for script_line in command.script:
            f.write(f"{script_line}\n")
    return working_dir, script_file


def execute_command(rc: OtlRC, command: Command) -> CResult:
    working_dir, script_file = create_scaffolding(rc, command)
    log_file = working_dir / "command.log"
    with log_file.open('w') as f:
        start_time = time.time()
        completed_proc = subprocess.run(['bash', str(script_file)],
                                        stdout=f, stderr=subprocess.STDOUT)
        end_time = time.time()

    execution_time_ms = (end_time - start_time) * 1000

    return dict(return_code=completed_proc.returncode, time=execution_time_ms)


def execute_command_list(commands: List[Command], rc: OtlRC) -> List[CResult]:
    """
    This is a naive executor for command lists -- it can not handle dependencies
    and will bark if you try to pass it any
    """
    for command in commands:
        if len(command.depdenencies) != 0:
            raise RuntimeError(
                "We don't support depdenencies for the naive executor! Please bother james@silogy.io about this")

    with Pool(processes=rc.jobs) as pool:
        bound_exec = partial(execute_command, rc)
        results = pool.map(bound_exec, commands)
    print(f"Executed {len(commands)} commands!")
