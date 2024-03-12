load("@toolchains//:otl_py.bzl", "OtlPythonToolchainInfo")
load("@root//pyrules:py_lib_rule.bzl", "OtlLibSrcs")


def _otl_simulator_rule(ctx: AnalysisContext) -> list[Provider]:
    run_tree_inputs = {}
    run_tree_recorded_deps = {}  # For a better error message when files collide
    for dep in ctx.attrs.otl_libs:
        dep_srcs = dep[OtlLibSrcs].srcs
        for src in dep_srcs:
            if (
                src.short_path in run_tree_recorded_deps
                and src != run_tree_inputs[src.short_path]
            ):
                original_dep = run_tree_recorded_deps[src.short_path]
                fail(
                    "dependency `{}` and `{}` both declare a source file named `{}`, consider renaming one of these files to avoid collision".format(
                        original_dep.label, dep.label, src.short_path
                    )
                )
            run_tree_inputs[src.short_path] = src
            run_tree_recorded_deps[src.short_path] = dep

    run_tree = ctx.actions.symlinked_dir("__%s__" % ctx.attrs.name, run_tree_inputs)

    out = ctx.actions.declare_output(ctx.attrs.output)
    cmd = cmd_args()

    cmd.add("/usr/bin/env")
    cmd.add(cmd_args(run_tree, format="PYTHONPATH={}"))
    cmd.add(
        cmd_args(
            [
                ctx.attrs.py_toolchain[OtlPythonToolchainInfo].interpreter,
                ctx.attrs.ex_script,
                cmd_args(
                    [
                        "--args",
                        cmd_args(
                            "{ ",
                            [
                                cmd_args('"', k,'"', " : ", '"', v, '"', delimiter="") for k, v in ctx.attrs.args.items()
                            ],
                            " }",
                            delimiter="",
                        ),
                    ]
                ),
                "--outputs",
                out.as_output(),
            ]
        )
    )

    ctx.actions.run(cmd, category="call_py")

    return [
        DefaultInfo(default_output=out),
        RunInfo(args=cmd_args(out)),
    ]


otl_simulator = rule(
    impl=_otl_simulator_rule,
    attrs={
        "output": attrs.string(),
        "ex_script": attrs.source(),
        "args": attrs.dict(
            key=attrs.string(), value=attrs.one_of(attrs.source(), attrs.string())
        ),
        "otl_libs": attrs.list(
            attrs.dep(providers=[OtlLibSrcs]), default=["//:otl_lib"]
        ),
        "py_toolchain": attrs.toolchain_dep(default="toolchains//:otl_py"),
    },
)
