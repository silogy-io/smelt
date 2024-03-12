OtlPythonToolchainInfo = provider(
    fields={"interpreter": provider_field(typing.Any, default=None)}
)


def _otl_py_bootstrap(ctx) -> list[Provider]:
    return [
        DefaultInfo(),
        OtlPythonToolchainInfo(interpreter=ctx.attrs.interpreter),
    ]


_PY_INTERPRETER = "python3"


otl_py_toolchain = rule(
    impl=_otl_py_bootstrap,
    attrs={
        "interpreter": attrs.string(default=_PY_INTERPRETER),
    },
    is_toolchain_rule=True,
)
