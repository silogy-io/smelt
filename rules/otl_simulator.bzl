load("@toolchains//:otl_py.bzl", "OtlPythonToolchainInfo")
load("@root//pyrules:py_lib_rule.bzl", "OtlLibSrcs")
load(":otl_utils.bzl", "create_py_lib_tree")


OtlSimulator = provider(fields={"simulator" : provider_field(typing.Any, default=None)})




def _otl_simulator_rule(ctx: AnalysisContext) -> list[Provider]:
    run_tree = create_py_lib_tree(ctx)
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
                                cmd_args('"', k, '"', " : ", '"', v, '"', delimiter="")
                                for k, v in ctx.attrs.args.items()
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
        OtlSimulator(simulator=out),
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
