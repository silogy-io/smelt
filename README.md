# otl

![GitHub release (latest by date)](https://img.shields.io/github/v/release/silogy-io/otl)
![GitHub](https://img.shields.io/github/license/silogy-io/otl)
![GitHub Workflow Status](https://img.shields.io/github/workflow/status/silogy-io/otl/smoke)

otl is a library for describing, running and tracking integration tests. It was built specifically for the testing of digital circuits.

At its core, otl is a test runner in the spirit of Make or Task. Each test is described as a series of bash commands, run in sequence.

⚠️ OTL IS UNDER ACTIVE DEVELOPMENT⚠️ feel free to use it, but docs and features are still being created rapidly, so there are no guarantees of stability :)

## Getting started

First, install otl with pip:

```
pip install pyotl
```

now create an otl file, named `tests.otl.yaml` -- below is an example

```yaml
# tests.otl.yaml
- name: test_example_1
  rule: raw_bash
  rule_args:
    cmds:
      # replace 'cmds' with whatever bash commands you want
      - echo "test1"

- name: test_example_2
  rule: raw_bash
  rule_args:
    cmds:
      - echo "test2"

- name: test_example_3
  rule: raw_bash
  rule_args:
    cmds:
      - echo "test3"
```

otl files can be validated via `otl validaite tests.otl.yaml #replace with the path to your command file`.

To execute all tests in a file, execute `otl execute tests.otl.yaml`
