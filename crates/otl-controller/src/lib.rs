use futures::{stream::FuturesUnordered, StreamExt};
use otl_data::client_commands::ClientCommand;
use std::{future::Future, sync::Arc};

use otl_client::Subscriber;
use otl_graph::{spawn_otl_server, CommandGraph, OtlServerHandle};
use tokio::sync::mpsc::{channel, Receiver, Sender, UnboundedSender};

struct SubscriberCtx {
    subscribers: Vec<Box<dyn Subscriber>>,
    send_chan: Sender<Box<dyn Subscriber>>,
    recv_chan: Receiver<Box<dyn Subscriber>>,
}

impl<'a> SubscriberCtx {
    fn new(subscribers: Vec<Box<dyn Subscriber>>) -> Self {
        let (send_chan, recv_chan) = channel(50);
        Self {
            subscribers,
            send_chan,
            recv_chan,
        }
    }
}

pub struct OtlControllerHandle {
    pub send_chan: Sender<Box<dyn Subscriber>>,
    pub tx_client: UnboundedSender<ClientCommand>,
}

pub struct OtlController {
    ctx: SubscriberCtx,
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
                        let val = self
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
