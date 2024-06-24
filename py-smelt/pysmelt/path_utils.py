import subprocess
import os


def memoize(func):
    cache = dict()

    def memoized_func():
        if "result" not in cache:
            cache["result"] = func()
        return cache["result"]

    return memoized_func


@memoize
def get_git_root() -> str:
    try:
        git_root = (
            subprocess.check_output(["git", "rev-parse", "--show-toplevel"])
            .strip()
            .decode("utf-8")
        )
        return git_root
    except subprocess.CalledProcessError:
        raise RuntimeError("This directory is not a git repository.")


def relatavize_inp_path(smelt_root: str, inp_path: str) -> str:
    """
    This function converts the input path, which is relative to the current working directory to be relative to `abs_path`
    """
    # Get the absolute path of the input path
    abs_inp_path = os.path.abspath(inp_path)

    # Return the path of `abs_inp_path` relative to `abs_path`
    rv = os.path.relpath(abs_inp_path, smelt_root)
    return rv
