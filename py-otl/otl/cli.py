from typing_extensions import Annotated
from pathlib import Path
import typer
import yaml

from otl.rc import OtlRC
from otl.importer import get_all_targets
from otl.otl_muncher import otl_to_command_list
from otl.serde import SafeDataclassDumper
from otl.pyotlexec.naive import execute_command_list

app = typer.Typer()


TlPath = Annotated[
    str,
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
    str,
    typer.Argument(
        exists=False,
        file_okay=True,
        dir_okay=False,
        writable=False,
        readable=True,
        resolve_path=True,
    ),
]


RulePath = Annotated[
    str,
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
def targets(
    rule_path: RulePath = "otl_rules", help="Prints out all visibile targets"
):
    otlrc = OtlRC.try_load()
    targets = get_all_targets(otlrc)

    print(targets)


@app.command()
def munch(
    otl_file: TlPath,
    output: CommandPath = "command.yaml",
    help="Converts .otl files to a command file",
):
    typer.echo(f"Validating: {otl_file}")
    otlrc = OtlRC.try_load()

    targets = get_all_targets(otlrc)
    commands = otl_to_command_list(test_list=otl_file, all_rules=targets, rc=otlrc)
    yaml.dump(commands, open(output, "w"), Dumper=SafeDataclassDumper, sort_keys=False)


@app.command()
def execute(
    otl_file: TlPath,
    help="Goes through the entireflow, from otl file to executing a command list",
):
    otlrc = OtlRC.try_load()
    targets = get_all_targets(otlrc)
    commands = otl_to_command_list(test_list=str(otl_file), all_rules=targets, rc=otlrc)
    execute_command_list(commands, otlrc)


def main():
    app()


if __name__ == "__main__":
    app()
