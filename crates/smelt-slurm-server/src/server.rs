use futures::StreamExt;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::mpsc::unbounded_channel};
use tokio_stream::wrappers::UnboundedReceiverStream;

use std::{net::SocketAddr, pin::Pin};

use smelt_data::{
    event_subscriber_server::EventSubscriber,
    Event, ExecutionFinish, ExecutionSubscribe, TaggedResult,
};

use scc::HashMap;
#[derive(Default)]
struct GlobalSlurmServer {
    senders: Arc<HashMap<String, tokio::sync::mpsc::UnboundedSender<Event>>>,
}

/// The handler for the HTTP request (this gets called when the HTTP GET lands at the start
/// of websocket negotiation). After this completes, the actual switching from HTTP to
/// websocket protocol will occur.
/// This is the last point where we can extract TCP/IP metadata such as IP address of the client
/// as well as things from HTTP headers such as user-agent of the browser etc.
///
///
///
///

type EventStream = Pin<Box<dyn tokio_stream::Stream<Item = Result<Event, tonic::Status>> + Send>>;

#[tonic::async_trait]
impl EventSubscriber for GlobalSlurmServer {
    type SubscribeReceivedEventsStream = EventStream;
    async fn subscribe_received_events(
        &self,
        request: tonic::Request<ExecutionSubscribe>,
    ) -> std::result::Result<tonic::Response<EventStream>, tonic::Status> {
        let trace_id = request.into_inner().trace_id;
        let (send, rcv) = unbounded_channel();
        let _ = self.senders.insert_async(trace_id, send).await;

        let strm: EventStream = Box::pin(UnboundedReceiverStream::new(rcv).map(Ok));
        Ok(tonic::Response::new(strm))
    }
    async fn subscription_complete(
        &self,
        request: tonic::Request<ExecutionFinish>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let trace = request.into_inner().trace_id;
        self.senders.remove_async(&trace).await;
        Ok(tonic::Response::new(()))
    }
}

#[tonic::async_trait]
impl smelt_data::event_listener_server::EventListener for GlobalSlurmServer {
    async fn send_event(
        &self,
        request: tonic::Request<Event>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let inner_event = request.into_inner();
        let rv = self
            .senders
            .get(&inner_event.trace_id)
            .map(|val| {
                tracing::trace!("fwding event {inner_event:?}");
                let _ = val.get().send(inner_event);
                tonic::Response::new(())
            })
            .ok_or(tonic::Status::new(
                tonic::Code::NotFound,
                "No trace was started with this traceid ",
            ));

        rv
    }
    async fn send_outputs(
        &self,
        request: tonic::Request<TaggedResult>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        let val = request.into_inner();
        tracing::warn!("tagged result payload is {val:?}");

        Ok(tonic::Response::new(()))
    }
}

pub async fn create_server(addr: SocketAddr, nonblocking: bool) -> Option<SocketAddr> {
    let senders = Arc::new(HashMap::new());

    let grpc = tonic::transport::Server::builder()
        .add_service(smelt_data::event_listener_server::EventListenerServer::new(
            GlobalSlurmServer {
                senders: senders.clone(),
            },
        ))
        .add_service(
            smelt_data::event_subscriber_server::EventSubscriberServer::new(GlobalSlurmServer {
                senders: senders.clone(),
            }),
        );

    let listener = TcpListener::bind(addr)
        .await
        .expect("Could not bind {addr} for server)");
    let local_addr = listener.local_addr().ok();
    let srv_ftr =
        grpc.serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener));
    //tracing::trace!("Listening on {local_addr:?}");
    println!("Listening on {local_addr:?}");
    if nonblocking {
        tokio::spawn(srv_ftr);
    } else {
        srv_ftr.await.expect("failed to serve future");
    };
    local_addr
}
