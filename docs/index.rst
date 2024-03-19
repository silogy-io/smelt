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



otl is a declaritive test framework to declare and run tests easily. 

otl is uses on two configuration files to define tests -- target lists and command lists.



At a high level, data flows through the system as shown below: 



``` bash 





┌────────────┐                 ┌───────────┐            
│            │                 │rule       │            
│target file │─────────────┐   │definitions│            
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
                           │   
                           │   
                           ▼   
                     ┌────────────┐
                     │command list│  ----->
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











Target Files
=========
Each target list is, as the name might suggest, composed of a list of targets. A target is the "atom" otl -- it represents a sequence of bash commands. A simple example is shown below: 


``` yaml

- name :spi_seed_1000:
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


Each target is composed of: 

  * name: a text identifier that must be unique within the target file. Names must be 'path' friendly, so some characters, like '/' are disallowed
  * rule: a rule references a python class, that contains the logic for generating bash commands to actually run a test. Rules will be described below
  * rule args: Arguments that are passed to a rule, at instantiation.


Python Rules
------------

Python define the mechanism for executing a target. The rule that is invoked above is defined as follows: 

``` python 
from dataclasses import dataclass, field
from otl.interfaces import Target, OtlPath
from typing import List, Dict


@dataclass
class run_spi(Target):
    """
    sanity test -- will move this to examples, eventually
    """

    seed: int

    def gen_script(self) -> List[str]:
        return ['echo "hello world"']

    def get_outputs(self) -> Dict[str, OtlPath]:
        return {}

``` 


A Rule is any python dataclass that inherits from the `Target` interface.  The target interface is implemented at otl/interfaces/target.py ,and is shown below:


```` python 
@dataclass
class Target(ABC):
    name: str

    def get_outputs(self) -> Dict[str, OtlPath]:
        ...

    def gen_script(self) -> List[str]:
        ...

    @staticmethod
    def rule_type() -> OtlTargetType:
        return OtlTargetType.Test

    def runtime_env_vars(self) -> Dict[str, str]:
        return {}

    def runtime_requirements(self) -> RuntimeRequirements:
        return RuntimeRequirements.default()

    def target_type(self) -> OtlTargetType:
        return OtlTargetType.Test

    def dependencies(self) -> List[TargetRef]:
        return []
``` 


When a target list is parsed 







Command Files
=============
Command files are used in conjunction with .otl files to define the commands that should be run as part of the tests. They provide a flexible way to customize the behavior of the tests and can include any command that can be run in a shell.



