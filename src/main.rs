mod config;
mod logger;
mod providers;

use bytes::Bytes;
use http::header::HOST;
use http::HeaderValue;
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use hyper::client::conn::http1 as client;
use hyper::server::conn::http1 as server;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use miette::Result;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;
use tracing::{debug, error};

#[derive(Debug)]
enum EventStream {
    PageView(String),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::parse();
    logger::init(&cfg.log_severity);
    providers::init(cfg.providers);

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

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);

        let sender = tx.clone();
        tokio::task::spawn(async move {
            server::Builder::new()
                .serve_connection(io, service_fn(|req| proxy(req, sender.to_owned())))
                .with_upgrades()
                .await
                .map_err(|err| error!(%err, "Failed to serve connection"))
        });
    }
}

async fn proxy(
    mut req: Request<hyper::body::Incoming>,
    tx: Sender<EventStream>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    debug!(method = %req.method(), uri = %req.uri(), "Request");

    let host = "recoeur.github.io";
    let port = 80;

    req.headers_mut()
        .insert(HOST, HeaderValue::from_str(host).unwrap());

    let stream = TcpStream::connect((host, port)).await.unwrap();
    let io = TokioIo::new(stream);

    let (mut sender, conn) = client::Builder::new().handshake(io).await.unwrap();

    tokio::task::spawn(async move { conn.await.map_err(|err| error!(%err, "Connection failed")) });

    let uri = req.uri().to_string();
    let res = sender.send_request(req).await.unwrap().map(|r| r.boxed());

    tx.send(EventStream::PageView(uri))
        .await
        .map_err(|err| error!(%err, "failed to send event"))
        .unwrap();

    Ok(res)
}
