.. otl documentation master file, created by
   sphinx-quickstart on Tue Mar 19 13:28:06 2024.
   You can adapt this file completely to your liking, but it should at least
   contain the root `toctree` directive.

Welcome to otl's documentation!
===============================

.. toctree::
   :maxdepth: 2
   :caption: Contents:



Indices and tables
==================

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`



otl is a declaritive task runner, designed specifically to declare and run tests for eda easily. 

otl uses two abstractions to define tasks -- targets and commands. at a high level, targets are used to represent common tasks succintly. At a lower level, commands describe explicitly every bash command that needs to be executed to run a test.



Abstractions
=============

Tasks
-----

A task is an abstraction to describe one unit of "work" under otl. It is
analogous to a make target in Gnu Make. A target and a command both represent a
task, at differing levels of abstraction 

Targets
-------
A target is a high level abstraction of a task under otl -- it seeks to provide
a programmable interface to paramterize tasks, using python 


a target definition is often called a `rule` in our documentation, following the
same naming scheme as bazel and buck2. below is a simple rule, to run a spi test
with paramterizable seed 


``` python 
from dataclasses import dataclass, field
from otl.interfaces import Target, OtlPath
from typing import List, Dict


@dataclass
class run_spi(Target):
    """
    example of running a spi test bench
    """

    seed: int

    def gen_script(self) -> List[str]:
        return [f'${GIT_ROOT}/test/run_spi_test.sh --seed {self.seed}']

``` 


The only method that a rule must implement is `gen_script` -- this method
describes the list of bash commands that compose a task. 

targets are instantiated in `target lists` with the following syntax:


``` yaml

- name: spi_seed_1000:
  rule: run_spi
  rule_args:
    seed: 1000

- name: spi_seed_1500:
  rule: run_spi
  rule_args:
    seed: 1500

- name: spi_seed_2000:
  rule: run_spi
  rule_args:
    seed: 2000

```

This file directly maps to the instantiation of the following python code:

``` python 

spi_seed_1000 = run_spi(name="spi_seed_1000", seed=1000)
spi_seed_1500 = run_spi(name="spi_seed_1500", seed=1500)
spi_seed_2000 = run_spi(name="spi_seed_2000", seed=2000)

```

Targets are lowered into commands, which can be actually executed by otlexec 

Commands
--------

A command explicitly describes the behaviour of a task, and will describe:

  * the bash commands that need to be executed 
  * depdendencies of a task -- which tasks need to be executed before this task
    can be executed 
  * outputs: files that have been explicitly declared by this task -- a task may
    create outputs that have not been declared
  * runtime requirements: describes the runtime requirements in terms of cpu,
    memory usage, timeout and environment variables

Below is an example list of commands that would be created from the `run_spi`
targets explained in the previous section 

- name: spi_seed_1000
  target_type: test
  script:
  - ${GIT_ROOT}/test/run_spi_test.sh --seed 1000
  depdenencies: []
  outputs: []
  runtime:
    num_cpus: 1
    max_memory_mb: 1024
    timeout: 600
    env: {"GIT_ROOT": /path/to/root}
- name: spi_seed_1500
  target_type: test
  script:
  - ${GIT_ROOT}/test/run_spi_test.sh --seed 1500
  depdenencies: []
  outputs: []
  runtime:
    num_cpus: 1
    max_memory_mb: 1024
    timeout: 600
    env: {"GIT_ROOT": /path/to/root}
- name: spi_seed_2000
  target_type: test
  script:
  - ${GIT_ROOT}/test/run_spi_test.sh --seed 2000
  depdenencies: []
  outputs: []
  runtime:
    num_cpus: 1
    max_memory_mb: 1024
    timeout: 600
    env: {"GIT_ROOT": /path/to/root}

Each Command maps to exactly one Target -- functionally, Commands should be
viewed as a "lowered" task representation, the same way that C or C++ code can
be lowered into LLVM bitcode. 

data flows through otl as shown below:


``` bash 

┌────────────┐                 ┌───────────┐            
│            │                 │rules      │            
│target list │─────────────┐   │           │            
│            │             │   └┬──────────┘            
└────────────┘             │    │
                           │    │
                           ▼    ▼
                     ┌────────────┐
                     │ otl parser │
                     │            │
                     └────────────┘
                           │   
                           │   
                           │     -----> emits a serialized list of commands
                           │   
                           ▼   
                     ┌────────────┐
                     │command list│  
                     └─────┬──────┘
                           │
                           ▼
                     ┌────────────┐
                     │ otlexec    │
                     └─────┬──────┘
                           │
                           │
                           ▼
                     ┌───────────────────────────────────────┐
                     │your tests have been executed          │
                     │good job!                              │
                     └───────────────────────────────────────┘

```



otl parser
=============
The otl parser is the software component that converts target lists into command
lists. 

The responsibility of this component is to check that a target list is well
formed. Functionally, this will parse a target list and then execute it 


otlexec
=============
Command files 





