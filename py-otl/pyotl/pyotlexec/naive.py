from typing import List, Dict
from multiprocessing import Pool
from pathlib import Path
import subprocess
import time
from functools import partial

from pyotl.rc import OtlRC
from pyotl.path_utils import get_git_root
from pyotl.interfaces.command import CResult, Command


def create_scaffolding(command: Command):
    env = command.runtime.env
    working_dir = Path(env["TARGET_ROOT"])
    script_file = working_dir / "command.sh"
    working_dir.mkdir(parents=True, exist_ok=True)
    with script_file.open("w") as f:
        for env_name, env_val in env.items():
            f.write(f"export {env_name}={env_val}\n")
        for script_line in command.script:
            f.write(f"{script_line}\n")
    return working_dir, script_file


def execute_command(command: Command) -> CResult:
    working_dir, script_file = create_scaffolding(command)
    log_file = working_dir / "command.log"
    with log_file.open("w") as f:
        start_time = time.time()
        completed_proc = subprocess.run(
            ["bash", str(script_file)], stdout=f, stderr=subprocess.STDOUT
        )
        end_time = time.time()

    execution_time_ms = (end_time - start_time) * 1000

    return dict(return_code=completed_proc.returncode, time=execution_time_ms)


def execute_command_list(commands: List[Command], rc: OtlRC) -> List[CResult]:
    """
    This is a naive executor for command lists -- it can not handle dependencies
    and will bark if you try to pass it any
    """
    for command in commands:
        if len(command.dependencies) != 0:
            raise RuntimeError(
                "We don't support depdenencies for the naive executor! File an issue against james@silogy.io about this"
            )

    with Pool(processes=rc.jobs) as pool:
        bound_exec = partial(execute_command, rc)
        results = pool.map(bound_exec, commands)
    print(f"Executed {len(commands)} commands!")
