import subprocess


def memoize(func):
    cache = dict()

    def memoized_func():
        if 'result' not in cache:
            cache['result'] = func()
        return cache['result']
    return memoized_func


@memoize
def get_git_root():
    try:
        git_root = subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"]).strip().decode('utf-8')
        return git_root
    except subprocess.CalledProcessError:
        print("This directory is not a git repository.")
        return None
