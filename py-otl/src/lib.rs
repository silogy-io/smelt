use anyhow::anyhow;
use otl_client::Subscriber;
use otl_controller::{spawn_otl_with_server, OtlControllerHandle};
use otl_core::OtlErr;
use otl_data::client_commands::ClientCommand;
use otl_data::{client_commands::ConfigureOtl, Event};

use prost::Message;
use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyType},
};

use std::{io::Read, sync::Arc};
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver, UnboundedSender};

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
    handle: OtlControllerHandle,
}

#[pyclass]
pub struct PySubscriber {
    recv_chan: UnboundedReceiver<Arc<Event>>,
    client_handle: UnboundedSender<ClientCommand>,
}

struct PySubscriberFwd {
    send_chan: UnboundedSender<Arc<Event>>,
}

impl PySubscriber {
    pub(crate) fn create_subscriber(
        client_handle: UnboundedSender<ClientCommand>,
    ) -> (PySubscriber, Box<PySubscriberFwd>) {
        let (send_chan, recv_chan) = tokio::sync::mpsc::unbounded_channel();
        (
            PySubscriber {
                recv_chan,
                client_handle,
            },
            Box::new(PySubscriberFwd { send_chan }),
        )
    }
}

#[async_trait::async_trait]
impl Subscriber for PySubscriberFwd {
    async fn recv_event(&mut self, event: Arc<Event>) -> anyhow::Result<()> {
        self.send_chan.send(event).map_err(|e| anyhow!(e))
    }
}

fn client_channel_err(_in_err: impl std::error::Error) -> PyErr {
    PyRuntimeError::new_err("Channel error trying to send a command to the client")
}

#[pymethods]
impl PyController {
    #[new]
    #[classmethod]
    pub fn new(_cls: Bound<'_, PyType>, serialized_cfg: Vec<u8>) -> PyResult<Self> {
        let cfg: ConfigureOtl =
            ConfigureOtl::decode(serialized_cfg.as_slice()).expect("Malformed cfg message");

        let handle = spawn_otl_with_server(cfg);
        Ok(PyController { handle })
    }

    pub fn set_graph(&self, graph: String) -> PyResult<()> {
        self.handle
            .tx_client
            .send(ClientCommand::send_graph(graph))
            .map_err(client_channel_err)?;
        Ok(())
    }

    pub fn run_all_tests(&self, tt: String) -> PyResult<()> {
        self.handle
            .tx_client
            .send(ClientCommand::execute_type(tt))
            .map_err(client_channel_err)?;
        Ok(())
    }

    pub fn run_one_test(&self, test: String) -> PyResult<()> {
        self.handle
            .tx_client
            .send(ClientCommand::execute_command(test))
            .map_err(client_channel_err)?;
        Ok(())
    }

    pub fn run_many_tests(&self, tests: Vec<String>) -> PyResult<()> {
        self.handle
            .tx_client
            .send(ClientCommand::execute_many(tests))
            .map_err(client_channel_err)?;
        Ok(())
    }

    pub fn add_py_listener(&self) -> PyResult<PySubscriber> {
        let (sub, fwder) = PySubscriber::create_subscriber(self.handle.tx_client.clone());
        self.handle
            .send_chan
            .blocking_send(fwder)
            .map_err(|_| PyRuntimeError::new_err("Could not add subscriber"))?;
        Ok(sub)
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

    pub fn send_client_message(&mut self, client_message: Bound<'_, PyBytes>) -> Result<(), PyErr> {
        let val = ClientCommand::decode(client_message.as_bytes())
            .map_err(|_| PyRuntimeError::new_err("Could not deserialize client message"))?;
        self.client_handle.send(val).map_err(client_channel_err)?;
        Ok(())
    }
}
