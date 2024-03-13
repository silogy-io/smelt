load("@root//pyrules:py_lib_rule.bzl", "OtlLibSrcs")

def create_py_lib_tree(ctx:AnalysisContext) -> Artifact:
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
    return run_tree

