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

mod ticker {
    /*
     * This is verbatim ripped from buck2, so i am keeping the license
     *
     * We might not want to do any rendering on the subscriber side, because I've decided to build
     * the console in python to start
     *
     * I did not link from buck2 source because we can just fork it here
     *
     *
     * Copyright (c) Meta Platforms, Inc. and affiliates.
     *
     * This source code is licensed under both the MIT license found in the
     * LICENSE-MIT file in the root directory of this source tree and the Apache
     * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
     * of this source tree.
     */

    use std::time::Duration;

    use tokio::time;
    use tokio::time::Instant;
    use tokio::time::Interval;
    use tokio::time::MissedTickBehavior;

    use otl_client::Tick;

    /// A simple wrapper around a [Interval] that tracks information about start/elapsed time and tick numbers. Note
    /// that ticks are not necessarily sequential, some may be skipped (and this indicates that ticks are running
    /// slower than requested).
    pub(crate) struct Ticker {
        interval: Interval,
        start_time: Instant,
    }

    impl Ticker {
        pub(crate) fn new(ticks_per_second: u32) -> Self {
            let interval_duration = Duration::from_secs_f64(1.0 / (ticks_per_second as f64));
            let mut interval = time::interval(interval_duration);
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            Self {
                interval,
                start_time: Instant::now(),
            }
        }

        pub(crate) async fn tick(&mut self) -> Tick {
            let current = self.interval.tick().await;
            self.tick_at(current)
        }

        pub(crate) fn tick_now(&mut self) -> Tick {
            self.tick_at(Instant::now())
        }

        fn tick_at(&mut self, current: Instant) -> Tick {
            // For time::interval, the Instant is the target instant for that tick and so it's possible
            // on the first one for it to actually be earlier than our start time.
            let elapsed_time = current
                .checked_duration_since(self.start_time)
                .unwrap_or(Duration::ZERO);

            Tick {
                start_time: self.start_time.into_std(),
                elapsed_time,
            }
        }
    }
}
