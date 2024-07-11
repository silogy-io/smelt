# smelt

![GitHub release (latest by date)](https://img.shields.io/github/v/release/silogy-io/smelt)
![GitHub License](https://img.shields.io/github/license/silogy-io/smelt)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/silogy-io/smelt/postcommit.yml)

Smelt is a simple and extensible task runner optimized for chip development
workflows. Smelt makes it easy to programmatically define arbitrarily many test
variants, run those tests in parallel, and analyze their results. Smelt
provides simple and efficient workflows to both localized and distributed
compute contexts.

Smelt is distributed as a python package and can be installed via:

`pipx install pysmelt`

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

Learn more from the [docs](https://silogy-io.github.io/smelt/)
