mod config;
mod logger;
mod providers;
mod proxy;

use hyper::server::conn::http1 as server;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use miette::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
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
        let provider = Arc::clone(&provider);
        let proxy = proxy::Proxy::new(provider, tx.clone());

        tokio::task::spawn(async move {
            server::Builder::new()
                .serve_connection(io, service_fn(|req| proxy.handle(req)))
                .await
                .map_err(|err| error!(%err, "Failed to serve connection"))
        });
    }
}
