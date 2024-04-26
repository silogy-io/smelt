use otl_core::OtlErr;
use otl_events::Event;
use otl_graph::{Command, CommandOutput};
use otl_graph::{CommandGraph, OtlServerHandle};
use prost::Message;
use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyType},
};
use pythonize::depythonize_bound;

use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};

pub fn arc_err_to_py(otl_err: Arc<OtlErr>) -> PyErr {
    let otl_string = otl_err.to_string();
    PyRuntimeError::new_err(otl_string)
}

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((a + b).to_string())
}

/// A Python module implemented in Rust.
#[pymodule]
fn otl(_py: Python, m: Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sum_as_string, &m)?)?;
    m.add_class::<PyCommandOutput>()?;
    m.add_class::<GraphServerHandle>()?;
    m.add_class::<PyExecHandle>()?;
    Ok(())
}

#[pyclass]
pub struct GraphServerHandle {}

#[pyclass]
pub struct PyExecHandle {
    #[allow(unused)]
    output: GraphExecHandle,
    is_done: bool,
}

impl PyExecHandle {
    fn process_event<'py>(&mut self, py: Python<'py>, event: Event) -> Bound<'py, PyBytes> {
        if event.finished_event() {
            self.is_done = true;
        }
        let tmp = event.encode_to_vec();
        PyBytes::new_bound(py, tmp.as_slice())
    }
}

#[pymethods]
impl PyExecHandle {
    pub fn get_next<'py>(&mut self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.output
            .blocking_next()
            .map(move |event| self.process_event(py, event))
    }

    pub fn try_next<'py>(&mut self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.output
            .maybe_next_event()
            .map(move |event| self.process_event(py, event))
    }

    pub fn done(&self) -> bool {
        self.is_done
    }
}

#[pyclass]
pub struct PyCommandOutput {
    #[allow(unused)]
    output: CommandOutput,
}

#[pymethods]
impl GraphServerHandle {
    #[new]
    #[classmethod]
    pub fn new(_cls: Bound<'_, PyType>, yaml_contents: String) -> PyResult<Self> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        std::thread::spawn(|| {
            let rt = Builder::new_multi_thread()
                .worker_threads(4) // specify the number of threads here
                .enable_all()
                .build()
                .unwrap();

            let graph = rt.block_on(CommandGraph::new());
            rt.block_on(graph.eat_commands());
        });

        Ok(GraphServerHandle {
            graph,
            async_runtime: rt,
        })
    }

    //#[classmethod]
    //pub fn from_py_commands(
    //    _cls: Bound<'_, PyType>,
    //    list_of_commands: Bound<'_, PyAny>,
    //) -> PyResult<Self> {
    //    let rt = Builder::new_multi_thread()
    //        .worker_threads(4) // specify the number of threads here
    //        .enable_all()
    //        .build()
    //        .unwrap();

    //    let commands: Vec<Command> = depythonize_bound(list_of_commands)?;

    //    let graph = rt.block_on(CommandGraph::new(commands))?;

    //    Ok(SyncCommandGraph {
    //        graph,
    //        async_runtime: rt,
    //    })
    //}

    ////TODO: tt thould be a target type enum, havent looked to expose yet
    //pub fn run_all_tests(&self, tt: String) -> PyResult<PyExecHandle> {
    //    let alltestfut = self.graph.run_all_typed(tt);

    //    Ok(self
    //        .async_runtime
    //        .block_on(alltestfut)
    //        .map(|val| PyExecHandle {
    //            is_done: false,
    //            output: val,
    //        })?)
    //}

    //pub fn run_one_test(&self, test: String) -> PyResult<PyExecHandle> {
    //    let alltestfut = self.graph.run_one_test(test);

    //    self.async_runtime
    //        .block_on(alltestfut)
    //        .map(|val| PyExecHandle {
    //            is_done: false,
    //            output: val,
    //        })
    //        .map_err(arc_err_to_py)
    //}
}

#[cfg(test)]
mod tests {

    //fn file_to_vec(yaml_data: &str) -> Vec<Command> {
    //    let script: Result<Vec<Command>, _> = serde_yaml::from_str(yaml_data);
    //    script.unwrap()
    //}

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
