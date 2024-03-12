# A list of available rules and their signatures can be found here: https://buck2.build/docs/api/rules/


load("pyrules/py_lib_rule.bzl", "py_otl_library")
load("rules/otl_simulator.bzl", "otl_simulator")

py_otl_library(
  name="otl_lib",
  srcs = glob(["otl/**/*.py"])
)





otl_simulator(
  name="my_simulator",
  ex_script="otl/otl_targets/prebuilt_simulator.py",
  args={"prebuilt_sim_path" : "toy_simulator.sh"},
  output="the_simulator"
)






