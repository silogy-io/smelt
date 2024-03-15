
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

CommandPath = Annotated[
    Path,
    typer.Argument(
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
    targets = get_all_targets(otlrc)
    print(targets)


@ app.command()
def munch(otl_file: TlPath, output: CommandPath, help="Converts .otl files to a .command file"):
    typer.echo(f"Validating: {otl_file}")
    otlrc = OtlRC.try_load()
    targets = get_all_targets(otlrc)


@ app.command()
def execute(testlist: TlPath, help="Goes through the entire"):
    typer.echo(f"Executing: {testlist}")
    otlrc = OtlRC.try_load()


def main():
    app()


if __name__ == "__main__":
    app()
