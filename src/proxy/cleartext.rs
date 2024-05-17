use anyhow::Result;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::Platform;

pub async fn start(platform: Arc<Platform>) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], platform.config.http_port));
    let listener = TcpListener::bind(addr).await?;
    info!(port = addr.port(), "HTTP");

    loop {
        let (socket, _) = listener.accept().await?;
        let p = platform.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(socket);
            Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(|req| super::handle_request(p.clone(), req)))
                .await
                .map_err(|err| error!(?err, "Failed to serve connection"))
        });
    }
}
