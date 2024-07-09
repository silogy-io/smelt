from pysmelt.interfaces.procedural import import_as_target
from pysmelt.default_targets import test_group, raw_bash

for i in range(5):
    raw_bash(name=f"my_test_{i}", cmds=[f'echo "howdy partner from test {i}"'])
