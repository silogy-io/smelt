# smelt

![GitHub release (latest by date)](https://img.shields.io/github/v/release/silogy-io/smelt)
![GitHub License](https://img.shields.io/github/license/silogy-io/smelt)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/silogy-io/smelt/postcommit.yml)

smelt is a library for describing, running and tracking integration tests.

At its core, smelt is a test runner in the spirit of Make or Task. Each test is described as a series of bash commands. Smelt will take advantage of massive

⚠️ SMELT IS UNDER ACTIVE DEVELOPMENT⚠️ feel free to use it, but docs and features are still being created rapidly, so there are no guarantees of stability :)

## Getting started

Install smelt with pipx:

```
pipx install pysmelt
```

Create an smelt file, named `tests.smelt.yaml` -- below is an example

```yaml
# tests.smelt.yaml
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

To execute all tests in a file, execute `smelt execute tests.smelt.yaml`

smelt files can be validated via `smelt validaite tests.smelt.yaml #replace with the path to your command file`.
