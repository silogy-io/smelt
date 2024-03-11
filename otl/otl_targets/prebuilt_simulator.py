
from otl.interfaces.simulator import SimulatorTarget, SimulatorProvider
from dataclasses import dataclass
from typing import Optional
from argparse import ArgumentParser
import json
import shutil


@dataclass(frozen=True)
class PrebuiltSimulator(SimulatorTarget):
    name: str
    prebuilt_sim_path: str
    output_path: str = None

    def to_buck2_target(self) -> str:
        raise NotImplementedError

    @property
    def out_sim_path(self):
        return self.output_path if self.output_path else self.prebuilt_sim_path

    def outputs(self) -> SimulatorProvider:
        sim_provider = dict(simulator=self.out_sim_path)
        return sim_provider

    @staticmethod
    def generate_target(prebuilt_sim_path: str, out_sim_path: str):
        shutil.copy(prebuilt_sim_path, out_sim_path)


if __name__ == "__main__":
    parser = ArgumentParser(
        description="Parses out commands for generating a prebuilt simulator")
    parser.add_argument('--args', type=str, help='JSON string of arguments')

    args = parser.parse_args()

    # Convert the JSON string into a Python dictionary
    args_dict = json.loads(args.argstr)

    PrebuiltSimulator.generate_target(**args_dict)
