
from typing_extensions import Annotated
from pathlib import Path, PosixPath

from otl.importer import get_all_targets
import typer

from otl.rc import OtlRC

app = typer.Typer()


TlPath = Annotated[
    Path,
    typer.Argument(
        ...,
        exists=True,
        file_okay=True,
        dir_okay=False,
        writable=False,
        readable=True,
        resolve_path=True,
    ),
]


RulePath = Annotated[
    Path,
    typer.Argument(
        ...,
        exists=False,
        file_okay=True,
        dir_okay=True,
        writable=False,
        readable=True,
        resolve_path=True,
    ),
]


@app.command()
def init(rule_path: RulePath = "otl_rules"):
    OtlRC.init_rc()


@app.command()
def targets(rule_path: RulePath = "otl_rules", help="Prints out all visibile targets"):
    otlrc = OtlRC.try_load()
    rules_dir: Path = otlrc.abs_rules_dir
    targets = get_all_targets(rules_dir)
    print(targets)


@ app.command()
def munch(testlist: TlPath):
    typer.echo(f"Validating: {testlist}")


@ app.command()
def execute(testlist: TlPath):
    typer.echo(f"Executing: {testlist}")


def main():
    app()


if __name__ == "__main__":
    app()
