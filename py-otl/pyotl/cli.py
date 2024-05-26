from typing_extensions import Annotated
from pathlib import Path
import typer
import yaml
from pyotl.rc import OtlRC
from pyotl.importer import get_all_targets
from pyotl.interfaces import OtlTargetType
from pyotl.otl_muncher import parse_otl
from pyotl.pygraph import create_graph
from pyotl.serde import SafeDataclassDumper

from typing import Optional
from typer import Typer, Argument, Exit

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


@app.command()
def init(rule_path: RulePath = Path("otl_rules")):
    OtlRC.init_rc()


@app.command()
def targets(
    rule_path: RulePath = Path("otl_rules"), help="Prints out all visibile targets"
):
    otlrc = OtlRC.try_load()
    targets = get_all_targets(otlrc)

    print(targets)


def validate_type(value: str):
    if value not in OtlTargetType._value2member_map_:
        raise Exit(
            f'Invalid value for "--tt". Possible values are {", ".join(OtlTargetType._value2member_map_.keys())}'  # type: ignore
        )
    return value


@app.command()
def munch(
    otl_file: TlPath,
    output: CommandPath = Path("command.yaml"),
    help="Converts .otl files to a command file",
):
    typer.echo(f"Validating: {otl_file}")

    _, commands = parse_otl(test_list=str(otl_file))
    yaml.dump(commands, open(output, "w"), Dumper=SafeDataclassDumper, sort_keys=False)


@app.command()
def execute(
    otl_file: TlPath,
    tt: str = typer.Option("test", help="OTL target type", callback=validate_type),
    target_name: Optional[str] = typer.Option(None, help="Target name"),
    rerun: bool = typer.Option(False, help="Rerun the command", is_flag=True),
    help="Goes through the entire flow, from otl file to executing a command list",
):

    graph = create_graph(str(otl_file))
    if target_name:
        graph.run_one_test_interactive(target_name)
    else:
        graph.run_all_tests(tt)
    if rerun:
        graph.rerun()


def main():
    app()


if __name__ == "__main__":
    app()
