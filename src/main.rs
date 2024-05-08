mod config;
mod logger;
mod providers;

use bytes::Bytes;
use http::header::HOST;
use http::{HeaderValue, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty};
use hyper::client::conn::http1 as client;
use hyper::server::conn::http1 as server;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use miette::Result;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;
use tracing::{debug, error};

#[derive(Debug)]
pub enum EventStream {
    PageView(String),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::parse();
    logger::init(&cfg.log_severity);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<EventStream>(1024);
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            debug!(?event, "Received event");
        }
    });

    let addr = SocketAddr::from(([127, 0, 0, 1], cfg.http_port));
    let listener = TcpListener::bind(addr).await.unwrap();
    tracing::info!(
        http_port = cfg.http_port,
        https_port = cfg.https_port,
        log_severity = cfg.log_severity.as_str(),
        "Server started"
    );

    let provider = Arc::new(providers::load());

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);

        let sender = tx.clone();
        let provider = Arc::clone(&provider);
        tokio::task::spawn(async move {
            server::Builder::new()
                .serve_connection(
                    io,
                    service_fn(|req| {
                        let provider = Arc::clone(&provider);
                        proxy(req, sender.to_owned(), provider)
                    }),
                )
                .await
                .map_err(|err| error!(%err, "Failed to serve connection"))
        });
    }
}

async fn proxy(
    req: Request<hyper::body::Incoming>,
    tx: Sender<EventStream>,
    provider: Arc<providers::Provider>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    debug!(method = %req.method(), uri = %req.uri(), "Request");

    let (parts, body) = req.into_parts();
    let host = parts.headers.get(HOST).unwrap().to_str().unwrap();
    let port = 80;

    match provider.get(host) {
        Some(endpoint) => {
            debug!("Matched endpont: {}", endpoint.hostname);
            let backend = endpoint.get_backend(parts.uri.to_string()).unwrap();
            debug!("Forwarding request to: {}", backend.location);
            let addr = format!("{}:{}", backend.location, port);
            let stream = TcpStream::connect(addr).await.unwrap();

            let io = TokioIo::new(stream);

            let (mut sender, conn) = client::Builder::new().handshake(io).await.unwrap();

            tokio::task::spawn(async move {
                conn.await.map_err(|err| error!(%err, "Connection failed"))
            });

            let host = backend.location.as_str();

            let req = Request::builder()
                .method("GET")
                .header(HOST, host)
                .body(body)
                .unwrap();

            let uri = req.uri().to_string();
            let res = sender.send_request(req).await.unwrap().map(|r| r.boxed());

            tx.send(EventStream::PageView(uri))
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

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}
