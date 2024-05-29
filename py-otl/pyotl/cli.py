from typing_extensions import Annotated
from pathlib import Path
import typer
import yaml
from pyotl.output import otl_console
from pyotl.interfaces import OtlTargetType
from pyotl.otl_muncher import parse_otl
from pyotl.output_utils import pretty_print_tests
from pyotl.pygraph import create_graph
from pyotl.serde import SafeDataclassDumper
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
    if value not in OtlTargetType._value2member_map_:
        raise Exit(
            f'Invalid value for "--tt". Possible values are {", ".join(OtlTargetType._value2member_map_.keys())}'  # type: ignore
        )
    return value


@app.command(
    help="Lowers an otl file to a command file",
)
def lower(
    otl_file: TlPath,
    output: CommandPath = Path("command.yaml"),
):
    typer.echo(f"Validating: {otl_file}")

    _, commands = parse_otl(test_list=str(otl_file))
    yaml.dump(commands, open(output, "w"), Dumper=SafeDataclassDumper, sort_keys=False)


@app.command(
    help="Executes an otl file",
)
def execute(
    otl_file: TlPath,
    tt: str = typer.Option("test", help="OTL target type", callback=validate_type),
    target_name: Optional[str] = typer.Option(
        None, help="Target name -- if not provided, runs all the tests"
    ),
    rerun: bool = typer.Option(
        False, help="Rerun the commands that failed", is_flag=True
    ),
):

    graph = create_graph(str(otl_file))
    if target_name:
        graph.run_one_test_interactive(target_name)
    else:
        graph.run_all_tests(tt)
    if rerun:
        graph.rerun()


@app.command(
    help="Executes an otl file",
)
def validate(
    otl_file: TlPath,
    tt: str = typer.Option("test", help="OTL target type", callback=validate_type),
    target_name: Optional[str] = typer.Option(
        None, help="Target name -- if not provided, runs all the tests"
    ),
    rerun: bool = typer.Option(
        False, help="Rerun the commands that failed", is_flag=True
    ),
):

    graph = create_graph(str(otl_file))
    otl_console.print(f"[green] {otl_file.name} is valid")
    pretty_print_tests(graph)


def main():
    app()


if __name__ == "__main__":
    app()
