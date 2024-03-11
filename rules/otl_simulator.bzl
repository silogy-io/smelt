

def simulator_def(ctx: AnalysisContext) -> list[Provider]:
  simulator = ctx.attrs.sim
  binary = ctx.actions.declare_output(ctx.attrs.binary_path) 
  return [DefaultInfo(default_outputs=[binary])]




rule

