use std::net::SocketAddr;

use anyhow::Result;
use bytes::Bytes;
use http::{header::HOST, HeaderValue, Request, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::body::Incoming;
use hyper::client::conn::http1 as client;
use hyper::{server::conn::http1 as server, service::service_fn};
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::{net::TcpStream, sync::mpsc::Sender};
use tracing::{debug, error, info};

use crate::Platform;
use crate::{providers::Provider, EventStream};

pub async fn start(port: u16, platform: &Platform) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    info!(port = addr.port(), "Running HTTP Server");

    loop {
        let (socket, _) = listener.accept().await?;
        let io = TokioIo::new(socket);
        let proxy = Proxy::new(platform.provider.clone(), platform.tx.clone());
        tokio::spawn(async move {
            server::Builder::new()
                .serve_connection(io, service_fn(|req| proxy.handle(req)))
                .await
                .map_err(|err| error!(?err, "Failed to serve connection"))
        });
    }
}

type ProxyResult = Result<Response<BoxBody<Bytes, hyper::Error>>>;

pub struct Proxy {
    provider: Arc<Provider>,
    sender: Sender<EventStream>,
}

impl Proxy {
    pub fn new(provider: Arc<Provider>, sender: Sender<EventStream>) -> Self {
        Self { provider, sender }
    }

    pub async fn handle(&self, req: Request<Incoming>) -> ProxyResult {
        debug!(method=%req.method(), uri=%req.uri(), "Request");

        let method = req.method().clone();
        let original_headers = req.headers().clone();
        let (parts, body) = req.into_parts();
        let host = parts
            .headers
            .get(HOST)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        match self.provider.get(host) {
            Some(endpoint) => {
                debug!("Matched endpont: {}", endpoint.hostname);
                let backend = endpoint.get_backend(parts.uri.to_string()).unwrap();

                debug!("Forwarding request to: {}", backend.location);
                let (host, port) = parse_host(backend.location.as_str());
                let addr = format!("{}:{}", host, port);
                debug!("Connecting to: {}", addr);
                let stream = TcpStream::connect(addr).await.unwrap();
                let io = TokioIo::new(stream);
                let (mut sender, conn) = client::Builder::new().handshake(io).await.unwrap();

                tokio::task::spawn(async move {
                    conn.await.map_err(|err| error!(%err, "Connection failed"))
                });

                let host = backend.location.as_str();
                let mut req = Request::builder().method(method);
                let new_headers = req.headers_mut().unwrap();

                for (name, value) in original_headers.iter() {
                    new_headers.insert(name, value.to_owned());
                }

                new_headers.insert(HOST, HeaderValue::from_str(host).unwrap());

                let req = req.body(body).unwrap();
                let uri = req.uri().to_string();
                let res = sender.send_request(req).await.unwrap().map(|r| r.boxed());

                self.sender
                    .send(EventStream::PageView(uri))
                    .await
                    .map_err(|err| error!(%err, "failed to send event"))
                    .unwrap();

                Ok(res)
            }
            None => {
                let res = Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(empty())
                    .unwrap();
                Ok(res)
            }
        }
    }
}

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn parse_host(host: &str) -> (String, u16) {
    let parts: Vec<&str> = host.split(':').collect();
    let host = parts[0].to_string();
    let port = match parts.get(1) {
        Some(part) => part.parse::<u16>().unwrap_or(80),
        None => 80,
    };

    (host, port)
}
