# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.




def flatten(xss: list[list[typing.Any]]) -> list[typing.Any]:
    return [x for xs in xss for x in xs]

_PY_INTERPRETER = select({
    "DEFAULT": "python3"
#    "config//os:windows": "python",
})

PythonBootstrapToolchainInfo = provider(fields = {"interpreter": provider_field(typing.Any, default = None)})



PythonBootstrapSources = provider(fields = {"srcs": provider_field(typing.Any, default = None)})


def _otl_py_bootstrap_library_impl(ctx: AnalysisContext) -> list[Provider]:
    tree = {src.short_path: src for src in ctx.attrs.srcs}
    output = ctx.actions.symlinked_dir("__{}__".format(ctx.attrs.name), tree)
    return [
        DefaultInfo(default_output = output),
        PythonBootstrapSources(srcs = dedupe(flatten([ctx.attrs.srcs] + [dep[PythonBootstrapSources].srcs for dep in ctx.attrs.deps]))),
    ]

def _otl_py_bootstrap_binary_impl(ctx: AnalysisContext) -> list[Provider]:
    """
    Declares a Python binary that is intended to be used in scripts that
    bootstrap other aspects of the Buck2 prelude. Python bootstrap binaries do
    not use the Python toolchain and, as such, are highly restricted in what
    they can and can't do. In particular, bootstrap binaries can only depend on
    bootstrap libraries and can only consist of a single file.
    """
    run_tree_inputs = {}
    run_tree_recorded_deps = {}  # For a better error message when files collide
    for dep in ctx.attrs.deps:
        dep_srcs = dep[PythonBootstrapSources].srcs
        for src in dep_srcs:
            if src.short_path in run_tree_recorded_deps and src != run_tree_inputs[src.short_path]:
                original_dep = run_tree_recorded_deps[src.short_path]
                fail("dependency `{}` and `{}` both declare a source file named `{}`, consider renaming one of these files to avoid collision".format(original_dep.label, dep.label, src.short_path))
            run_tree_inputs[src.short_path] = src
            run_tree_recorded_deps[src.short_path] = dep

    run_tree = ctx.actions.symlinked_dir("__%s__" % ctx.attrs.name, run_tree_inputs)
    output = ctx.actions.copy_file(ctx.attrs.main.short_path, ctx.attrs.main)

    interpreter = ctx.attrs._python_bootstrap_toolchain[PythonBootstrapToolchainInfo].interpreter

    run_args = cmd_args()
    run_args.add("/usr/bin/env")
    run_args.add(cmd_args(run_tree, format = "PYTHONPATH={}"))
    run_args.add(interpreter)
    run_args.add(output)
    #TODO: support windows, eventually
    return [DefaultInfo(default_output = output), RunInfo(args = run_args)]


def _system_python_bootstrap_toolchain_impl(ctx):
    return [
        DefaultInfo(),
        PythonBootstrapToolchainInfo(interpreter = ctx.attrs.interpreter),
    ]

# Creates a new bootstrap toolchain using Python that is installed on your system.
# You may use it in your toolchain cell as follows:
#
# ```bzl
# load("@prelude//toolchains:python.bzl", "system_python_bootstrap_toolchain")
#
# system_python_bootstrap_toolchain(
#     name = "python_bootstrap", # the default name rules look for
#     visibility = ["PUBLIC"],
# )
# ```
system_python_bootstrap_toolchain = rule(
    impl = _system_python_bootstrap_toolchain_impl,
    attrs = {
        "interpreter": attrs.string(default = _INTERPRETER),
    },
    is_toolchain_rule = True,
)








py_otl_binary = rule(
  impl = _otl_py_bootstrap_binary_impl, 
  doc = "Python binary that is called within the otl buck2 framework",
  attrs = {
        "deps": attrs.list(attrs.dep(providers = [PythonBootstrapSources]), default = []),
        "main": attrs.source(),
        "args" : 
        "_exec_os_type": buck.exec_os_type_arg(),
        "_python_bootstrap_toolchain": toolchains_common.python_bootstrap(),
        
    },
)



py_otl_library = rule(
  impl = _otl_py_bootstrap_library_impl,
  attrs = { 
    "deps": attrs.list(attrs.dep(providers = [PythonBootstrapSources]), default = []),
    "srcs": attrs.list(attrs.source()),
  }
)
