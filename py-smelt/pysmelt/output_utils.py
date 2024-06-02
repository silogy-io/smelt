from rich.table import Table
from pysmelt.output import smelt_console
from pysmelt.pygraph import PyGraph


def pretty_print_tests(graph: PyGraph):
    """
    pretty print all the tests in the graph
    """
    commands = graph.commands
    table = Table(show_header=True, header_style="bold magenta")
    table.add_column("Command Name")
    table.add_column("Command type")

    # Add rows to the table
    for command in commands:
        if command.target_type == "test":
            table.add_row(command.name, command.target_type)
    smelt_console.print(table)
