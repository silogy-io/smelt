

OtlLibSrcs = provider(fields = {"srcs": provider_field(typing.Any, default = None)})


def flatten(xss: list[list[typing.Any]]) -> list[typing.Any]:
    return [x for xs in xss for x in xs]


def _otl_py_bootstrap_library_impl(ctx: AnalysisContext) -> list[Provider]:
    tree = {src.short_path: src for src in ctx.attrs.srcs}
    output = ctx.actions.symlinked_dir("__{}__".format(ctx.attrs.name), tree)
    return [
        DefaultInfo(default_output = output),
        OtlLibSrcs(srcs = dedupe(flatten([ctx.attrs.srcs] + [dep[OtlLibSrcs].srcs for dep in ctx.attrs.deps]))),
    ]




py_otl_library = rule(
  impl = _otl_py_bootstrap_library_impl,
  attrs = { 
    # TODO: may need to support prebuilt eventually
    "deps": attrs.list(attrs.dep(providers = [OtlLibSrcs]), default = []),
    "srcs": attrs.list(attrs.source()),
  }
)
