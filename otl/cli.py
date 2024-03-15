
from typing_extensions import Annotated
from pathlib import Path, PosixPath
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
    rule_path.absolute()


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
