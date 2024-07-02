use smelt_core::SmeltErr;
use smelt_data::client_commands::{client_resp::ClientResponses, ClientCommand, ClientResp};
use smelt_data::{client_commands::ConfigureSmelt, Event};

mod telemetry;
use telemetry::{get_subscriber, init_subscriber};

use std::sync::Once;

static START: Once = Once::new();

// run initialization here

use prost::Message;
use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyBytes, PyType},
};
use smelt_events::{ClientCommandBundle, ClientCommandResp, EventStreams};
use smelt_graph::{spawn_graph_server, SmeltServerHandle};

use std::sync::Arc;
use tokio::sync::mpsc::{error::TryRecvError, Receiver, UnboundedSender};

pub fn arc_err_to_py(smelt_err: Arc<SmeltErr>) -> PyErr {
    let smelt_string = smelt_err.to_string();
    PyRuntimeError::new_err(smelt_string)
}

/// A Python module implemented in Rust.
#[pymodule]
fn pysmelt(_py: Python, m: Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyController>()?;
    m.add_class::<PyEventStream>()?;
    Ok(())
}

#[pyclass]
pub struct PyController {
    handle: SmeltServerHandle,
}

#[pyclass]
pub struct PyEventStream {
    recv_chan: Receiver<Event>,
    done: bool,
}

impl PyEventStream {
    pub(crate) fn create_subscriber(recv_chan: Receiver<Event>) -> Self {
        Self {
            recv_chan,
            done: false,
        }
    }
}

fn client_channel_err(_in_err: impl std::error::Error) -> PyErr {
    PyRuntimeError::new_err("Channel error trying to send a command to the client")
}

fn handle_client_resp(
    resp: Result<ClientCommandResp, impl std::error::Error>,
) -> PyResult<ClientResp> {
    match resp {
        Ok(Ok(client_resp)) => Ok(client_resp),
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
        let cfg: ConfigureSmelt =
            ConfigureSmelt::decode(serialized_cfg.as_slice()).expect("Malformed cfg message");

        START.call_once(|| {
            let subscriber = get_subscriber("smelt".into(), "info".into(), std::io::stdout);
            init_subscriber(subscriber);
        });
        let handle = spawn_graph_server(cfg);
        Ok(PyController { handle })
    }

    pub fn set_graph(&self, graph: String) -> PyResult<()> {
        let EventStreams { sync_chan, .. } =
            submit_message(&self.handle.tx_client, ClientCommand::send_graph(graph))?;

        let resp = sync_chan.blocking_recv();
        handle_client_resp(resp).map(|_| ())
    }

    pub fn run_all_tests(&self, tt: String) -> PyResult<PyEventStream> {
        self.run_tests(ClientCommand::execute_type(tt))
    }

    pub fn run_one_test(&self, test: String) -> PyResult<PyEventStream> {
        self.run_tests(ClientCommand::execute_command(test))
    }

    pub fn run_many_tests(&self, tests: Vec<String>) -> PyResult<PyEventStream> {
        self.run_tests(ClientCommand::execute_many(tests))
    }

    pub fn get_current_cfg<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let command = ClientCommand::get_cfg();
        let EventStreams { sync_chan, .. } =
            submit_message(&self.handle.tx_client, command).map_err(client_channel_err)?;
        let resp = sync_chan.blocking_recv();
        handle_client_resp(resp).map(|val| match val.client_responses.unwrap() {
            ClientResponses::CurrentCfg(a) => to_bytes(a, py),
            _ => unreachable!("Incorrect response coming from the server for get_current_cfg"),
        })
    }
}

impl PyController {
    fn run_tests(&self, command: ClientCommand) -> PyResult<PyEventStream> {
        let EventStreams { event_stream, .. } =
            submit_message(&self.handle.tx_client, command).map_err(client_channel_err)?;
        Ok(PyEventStream::create_subscriber(event_stream))
    }
}

#[inline]
fn to_bytes<M: Message>(message: M, py: Python<'_>) -> Bound<'_, PyBytes> {
    let val = message.encode_to_vec();

    PyBytes::new_bound(py, &val)
}

#[pymethods]
impl PyEventStream {
    pub fn pop_message_blocking<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let val = self
            .recv_chan
            .blocking_recv()
            .ok_or_else(|| PyRuntimeError::new_err("Event channel closed"))?;
        self.set_done(&val);

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
                self.set_done(&val);
                let val = val.encode_to_vec();

                Ok(Some(PyBytes::new_bound(py, &val)))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(_) => Err(PyRuntimeError::new_err("Event channel closed")),
        }
    }

    /// Returns true if we've seen a entire Invocation complete end to end AND the channel has
    /// been closed
    pub fn is_done(&mut self, _py: Python<'_>) -> bool {
        self.done && self.recv_chan.is_closed()
    }
}

impl PyEventStream {
    fn set_done(&mut self, event: &Event) {
        if event.finished_event() {
            self.done = true;
        }
    }
}
