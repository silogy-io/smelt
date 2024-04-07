use command::CommandOutput;
use error::arc_err_to_py;
use graph::CommandGraph;
use pyo3::{prelude::*, types::PyType};
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
    graph: CommandGraph,
    async_runtime: Runtime,
}

#[pyclass]
pub struct PyCommandOutput {
    output: CommandOutput,
}

#[pymethods]
impl SyncCommandGraph {
    #[classmethod]
    pub fn create(cls: Bound<'_, PyType>, yaml_contents: String) -> PyResult<Self> {
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
}
