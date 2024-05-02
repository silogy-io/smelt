use futures::{stream::FuturesUnordered, StreamExt};
use otl_data::client_commands::ClientCommand;
use std::{future::Future, sync::Arc};

use otl_client::Subscriber;
use otl_graph::{spawn_otl_server, CommandGraph, OtlServerHandle};
use tokio::sync::mpsc::{channel, Receiver, Sender, UnboundedSender};

/// This struct owns all logic around subscribers that are present in the system
struct SubscriberCtx {
    /// Contains all of the subscribers that are present in the system
    subscribers: Vec<Box<dyn Subscriber>>,

    /// Channel where the context can receive new subscribers from -- if the client
    /// wants to register a new subscriber at runtime, it will be sent on this channel
    recv_chan: Receiver<Box<dyn Subscriber>>,

    /// Other half of the recv_chan above -- we keep it around so we can pass it out to clients
    /// easily -- perhaps this should be removed though
    send_chan: Sender<Box<dyn Subscriber>>,
}

impl SubscriberCtx {
    fn new(subscribers: Vec<Box<dyn Subscriber>>) -> Self {
        let (send_chan, recv_chan) = channel(50);
        Self {
            subscribers,
            send_chan,
            recv_chan,
        }
    }
}

/// Client side handle for the OtlController -- this is the struct that imperative code and end
/// users have access  to
pub struct OtlControllerHandle {
    /// Channel for submitting new subscribers to the controller
    pub send_chan: Sender<Box<dyn Subscriber>>,
    /// Used to kick off actual client commands like running many tests
    /// ClientCommand is the thin interface
    pub tx_client: UnboundedSender<ClientCommand>,
}

/// Top level struct that contains all state on the "server side"
pub struct OtlController {
    /// Holds all the subscriber logic for "responding" to Events
    ctx: SubscriberCtx,
    /// Holds handles for server -- allows us to receive events
    server_handle: OtlServerHandle,
}

pub fn spawn_otl_with_server() -> OtlControllerHandle {
    let (tx_client, rx_client) = tokio::sync::mpsc::unbounded_channel();
    let (tx_tele, rx_tele) = tokio::sync::mpsc::channel(100);

    let server_handle = OtlServerHandle { rx_tele, tx_client };
    let ctx = SubscriberCtx::new(vec![]);
    let mut ctrl = OtlController { ctx, server_handle };
    let handle = ctrl.handle();

    use tokio::runtime::Builder;

    std::thread::spawn(move || {
        let rt = Builder::new_multi_thread()
            .worker_threads(4) // specify the number of threads here
            .enable_all()
            .build()
            .unwrap();

        //todo -- add failure handling here
        let mut graph = rt.block_on(CommandGraph::new(rx_client, tx_tele)).unwrap();
        rt.block_on(async move {
            // if either of these futures exit, we should head out
            tokio::select! {
                _graph = graph.eat_commands() => {}
                _ctrl = ctrl.runtime_loop() => {}
            }
        });
    });
    handle
}

impl Default for OtlController {
    fn default() -> Self {
        Self::new()
    }
}

impl OtlController {
    pub fn handle(&self) -> OtlControllerHandle {
        OtlControllerHandle {
            send_chan: self.ctx.send_chan.clone(),
            tx_client: self.server_handle.tx_client.clone(),
        }
    }
    pub fn new() -> Self {
        Self::new_with_subscribers(vec![])
    }

    pub fn new_with_subscribers(subscribers: Vec<Box<dyn Subscriber>>) -> Self {
        let server_handle = spawn_otl_server();
        let ctx = SubscriberCtx::new(subscribers);
        OtlController { ctx, server_handle }
    }

    pub fn add_subscriber(&mut self, subscriber: impl Into<Box<dyn Subscriber>>) {
        self.ctx.subscribers.push(subscriber.into())
    }

    pub async fn runtime_loop(&mut self) {
        loop {
            // right now, this select is not required,
            // but i am keeping it because its good scaffolding in case we want to process other
            // events in parallel in the runtime loop
            tokio::select! {
                val = self.server_handle.rx_tele.recv() => {
                    match val {
                        Some(val) => {
                        let event = Arc::new(val);
                        let _val = self
                            .for_each_subscriber(|subscriber| subscriber.recv_event(event.clone()))
                            .await;
                        }
                        None => {
                            dbg!("imout");
                            return;
                        }
                    }
                }
                sub = self.ctx.recv_chan.recv() => {
                    match sub {
                        Some(sub) => {
                            self.ctx.subscribers.push(sub)
                        }
                        None => {
                            return;
                        }
                    }


                }


            }
        }
    }

    pub(crate) async fn for_each_subscriber<'b, Fut>(
        &'b mut self,
        f: impl FnMut(&'b mut Box<dyn Subscriber + '_>) -> Fut,
    ) -> anyhow::Result<()>
    where
        Fut: Future<Output = anyhow::Result<()>> + 'b,
    {
        let mut futures: FuturesUnordered<_> = self.ctx.subscribers.iter_mut().map(f).collect();
        while let Some(res) = futures.next().await {
            res?;
        }
        Ok(())
    }
}
