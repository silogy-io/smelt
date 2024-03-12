#

from dataclasses import dataclass
from otl.stimulus import StimulusArtifact

from typing import Dict, List, Any


simulator = PrebuiltSimulator.from_binary()
benchmark = cpp_benchmark()

test = sim_test(simulator, binary)
