use command::{Command, CommandOutput};
use error::{arc_err_to_py, OtlErr};
use graph::{CommandGraph, CommandRef};
use pyo3::{
    prelude::*,
    types::{IntoPyDict, PyDict, PyList, PyType},
};
use pythonize::{depythonize_bound, pythonize_custom};
use tokio::runtime::{Builder, Runtime};
pub mod command;
pub mod error;
pub mod graph;

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((a + b).to_string())
}

/// A Python module implemented in Rust.
#[pymodule]
fn cweb(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    Ok(())
}

#[pyclass]
pub struct SyncCommandGraph {
    pub(crate) graph: CommandGraph,
    pub(crate) async_runtime: Runtime,
}

#[pyclass]
pub struct PyCommandOutput {
    output: CommandOutput,
}

impl Command {
    fn to_pycommand<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let otl_interfaces = py.import_bound("otl.interfaces")?;
        let command_type = otl_interfaces.getattr("Command")?;
        let runtime_reqs = otl_interfaces.getattr("RuntimeRequirements")?;
        let tt = otl_interfaces.getattr("TargetType")?;
        let tt_obj = tt.call((self.target_type.to_string(),), None)?;
        let runtime_req_args = pythonize::pythonize(py, &self.runtime)?;
        let runtime_req_kwaargs = runtime_req_args.downcast_bound::<PyDict>(py)?;
        let runtime_req_obj = runtime_reqs.call((), Some(runtime_req_kwaargs))?;
        let command_args = (
            self.name.clone(),
            tt_obj,
            self.script.clone(),
            self.outputs.clone(),
            runtime_req_obj,
        );
        let command_obj = command_type.call(command_args, None);
        command_obj
    }
}

#[pymethods]
impl SyncCommandGraph {
    #[new]
    #[classmethod]
    pub fn new(_cls: Bound<'_, PyType>, yaml_contents: String) -> PyResult<Self> {
        let rt = Builder::new_multi_thread()
            .worker_threads(4) // specify the number of threads here
            .enable_all()
            .build()
            .unwrap();

        let graph = rt.block_on(CommandGraph::from_commands_str(yaml_contents))?;

        Ok(SyncCommandGraph {
            graph,
            async_runtime: rt,
        })
    }

    #[classmethod]
    pub fn from_py_commands(
        _cls: Bound<'_, PyType>,
        list_of_commands: Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let rt = Builder::new_multi_thread()
            .worker_threads(4) // specify the number of threads here
            .enable_all()
            .build()
            .unwrap();

        let commands: Vec<Command> = depythonize_bound(list_of_commands)?;

        let graph = rt.block_on(CommandGraph::new(commands))?;

        Ok(SyncCommandGraph {
            graph,
            async_runtime: rt,
        })
    }

    pub fn run_all_tests(&self) -> PyResult<Vec<PyCommandOutput>> {
        let alltestfut = self.graph.run_all_tests();
        let vec = self
            .async_runtime
            .block_on(alltestfut)
            .into_iter()
            .map(|val| {
                val.map_err(|arc| arc_err_to_py(arc))
                    .map(|val| PyCommandOutput { output: val })
            })
            .collect::<PyResult<Vec<PyCommandOutput>>>();
        vec
    }
    pub fn run_one_test(&self, test: String) -> PyResult<PyCommandOutput> {
        let alltestfut = self.graph.run_one_test(test);
        let output = self
            .async_runtime
            .block_on(alltestfut)
            .map(|val| PyCommandOutput { output: val })
            .map_err(|arc| arc_err_to_py(arc));
        output
    }

    //#[getter]
    //pub fn build(&self) -> PyResult<Vec<Command>> {
    //    let gil: PyResult<PyList> = Python::with_gil(|py| {
    //        let otl_interfaces = py.import("otl.interfaces")?;
    //        let command_type = otl_interfaces.getattr("Command")?;
    //        let vec: Vec<PyDict> = self
    //            .graph
    //            .all_commands
    //            .iter()
    //            .map(|val| pythonize::pythonize_custom<PyDict>(val.0.as_ref().clone()))
    //            .collect()?;
    //    });

    //    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    fn file_to_vec(yaml_data: &str) -> Vec<Command> {
        let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data);
        script.unwrap()
    }

    //#[test]
    //fn obj_to_py() {
    //    let vals = file_to_vec(include_str!("../test_data/command_lists/cl1.yaml"));
    //    pyo3::prepare_freethreaded_python();

    //    Python::with_gil(|py| {
    //        let val: Vec<Bound<PyAny>> =
    //            vals.iter().map(|val| val.to_pycommand(py).unwrap()).collect();
    //    });
    //}
}
