from typing_extensions import Annotated
from pathlib import Path
import typer
import yaml
from pysmelt.output import smelt_console
from pysmelt.interfaces import SmeltTargetType
from pysmelt.smelt_muncher import parse_smelt
from pysmelt.output_utils import pretty_print_tests
from pysmelt.pygraph import create_graph
from pysmelt.serde import SafeDataclassDumper
from typing import Optional
from typer import Exit

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
        exists=False,
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


def validate_type(value: str):
    if value not in SmeltTargetType._value2member_map_:
        raise Exit(
            f'Invalid value for "--tt". Possible values are {", ".join(SmeltTargetType._value2member_map_.keys())}'  # type: ignore
        )
    return value


@app.command(
    help="Lowers an smelt file to a command file",
)
def lower(
    smelt_file: TlPath,
    output: CommandPath = Path("command.yaml"),
):
    typer.echo(f"Validating: {smelt_file}")

    _, commands = parse_smelt(test_list=str(smelt_file))
    yaml.dump(commands, open(output, "w"), Dumper=SafeDataclassDumper, sort_keys=False)


@app.command(
    help="Executes an smelt file",
)
def execute(
    smelt_file: TlPath,
    tt: str = typer.Option("test", help="SMELT target type", callback=validate_type),
    target_name: Optional[str] = typer.Option(
        None, help="Target name -- if not provided, runs all the tests"
    ),
    rerun: bool = typer.Option(
        False, help="Rerun the commands that failed", is_flag=True
    ),
):

    graph = create_graph(str(smelt_file))
    if target_name:
        graph.run_one_test_interactive(target_name)
    else:
        graph.run_all_tests(tt)
    if rerun:
        graph.rerun()


@app.command(
    help="Executes an smelt file",
)
def validate(
    smelt_file: TlPath,
    tt: str = typer.Option("test", help="SMELT target type", callback=validate_type),
    target_name: Optional[str] = typer.Option(
        None, help="Target name -- if not provided, runs all the tests"
    ),
    rerun: bool = typer.Option(
        False, help="Rerun the commands that failed", is_flag=True
    ),
):

    graph = create_graph(str(smelt_file))
    smelt_console.print(f"[green] {smelt_file.name} is valid")
    pretty_print_tests(graph)


def main():
    app()


if __name__ == "__main__":
    app()
