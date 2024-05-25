use otl_core::OtlErr;
use otl_data::client_commands::ClientCommand;
use otl_data::{client_commands::ConfigureOtl, Event};

use otl_events::{ClientCommandBundle, ClientCommandResp, EventStreams};
use otl_graph::{spawn_graph_server, OtlServerHandle};
use prost::Message;
use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyType},
};

use std::sync::Arc;
use tokio::sync::mpsc::{error::TryRecvError, Receiver, UnboundedSender};

pub fn arc_err_to_py(otl_err: Arc<OtlErr>) -> PyErr {
    let otl_string = otl_err.to_string();
    PyRuntimeError::new_err(otl_string)
}

/// A Python module implemented in Rust.
#[pymodule]
fn pyotl(_py: Python, m: Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyController>()?;
    m.add_class::<PySubscriber>()?;
    Ok(())
}

#[pyclass]
pub struct PyController {
    handle: OtlServerHandle,
}

#[pyclass]
pub struct PySubscriber {
    recv_chan: Receiver<Event>,
}

impl PySubscriber {
    pub(crate) fn create_subscriber(recv_chan: Receiver<Event>) -> Self {
        Self { recv_chan }
    }
}

fn client_channel_err(_in_err: impl std::error::Error) -> PyErr {
    PyRuntimeError::new_err("Channel error trying to send a command to the client")
}

fn handle_client_resp(resp: Result<ClientCommandResp, impl std::error::Error>) -> PyResult<()> {
    match resp {
        Ok(Ok(())) => Ok(()),
        Ok(Err(str)) => Err(PyRuntimeError::new_err(format!(
            "Client command failed with error {str}"
        ))),
        Err(err) => Err(PyRuntimeError::new_err(err.to_string())),
    }
}

fn submit_message(
    tx_client: &UnboundedSender<ClientCommandBundle>,
    message: ClientCommand,
) -> Result<EventStreams, PyErr> {
    let (bundle, recv) = ClientCommandBundle::from_message(message);

    tx_client.send(bundle).map_err(client_channel_err)?;
    Ok(recv)
}

#[pymethods]
impl PyController {
    #[new]
    #[classmethod]
    pub fn new(_cls: Bound<'_, PyType>, serialized_cfg: Vec<u8>) -> PyResult<Self> {
        let cfg: ConfigureOtl =
            ConfigureOtl::decode(serialized_cfg.as_slice()).expect("Malformed cfg message");

        let handle = spawn_graph_server(cfg);
        Ok(PyController { handle })
    }

    pub fn set_graph(&self, graph: String) -> PyResult<()> {
        let EventStreams { sync_chan, .. } =
            submit_message(&self.handle.tx_client, ClientCommand::send_graph(graph))?;

        let resp = sync_chan.blocking_recv();
        handle_client_resp(resp)
    }

    pub fn run_all_tests(&self, tt: String) -> PyResult<PySubscriber> {
        self.run_tests(ClientCommand::execute_type(tt))
    }

    pub fn run_one_test(&self, test: String) -> PyResult<PySubscriber> {
        self.run_tests(ClientCommand::execute_command(test))
    }

    pub fn run_many_tests(&self, tests: Vec<String>) -> PyResult<PySubscriber> {
        self.run_tests(ClientCommand::execute_many(tests))
    }
}

impl PyController {
    fn run_tests(&self, command: ClientCommand) -> PyResult<PySubscriber> {
        let EventStreams { event_stream, .. } =
            submit_message(&self.handle.tx_client, command).map_err(client_channel_err)?;
        Ok(PySubscriber::create_subscriber(event_stream))
    }
}
#[pymethods]
impl PySubscriber {
    pub fn pop_message_blocking<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let val = self
            .recv_chan
            .blocking_recv()
            .ok_or_else(|| PyRuntimeError::new_err("Event channel closed unexpectedly"))?;

        let val = val.encode_to_vec();

        Ok(PyBytes::new_bound(py, &val))
    }
    pub fn nonblocking_pop<'py>(
        &mut self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyBytes>>> {
        let val = self.recv_chan.try_recv();

        match val {
            Ok(val) => {
                let val = val.encode_to_vec();

                Ok(Some(PyBytes::new_bound(py, &val)))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(_) => Err(PyRuntimeError::new_err("Event channel closed unexpectedly")),
        }
    }
}
