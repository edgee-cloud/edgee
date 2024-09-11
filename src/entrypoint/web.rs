use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;
use tracing::error;
use tracing::info;
use crate::config;
use super::handle_request;

pub async fn start() -> anyhow::Result<()> {
    let cfg = config::get().http.as_ref().ok_or_else(|| anyhow::anyhow!("HTTP configuration is missing"))?;

    info!(address = cfg.address, force_https = cfg.force_https, "Starting HTTP entrypoint");

    let addr: SocketAddr = cfg.address.parse()?;
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let service = RequestManager::new(remote_addr);
        tokio::spawn(async move {
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(io, service)
                .await
            {
                error!(?err, ?remote_addr, "failed to serve connections");
            }
        });
    }
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

struct RequestManager {
    remote_addr: SocketAddr,
}

impl RequestManager {
    fn new(addr: SocketAddr) -> Self {
        Self { remote_addr: addr }
    }
}

impl hyper::service::Service<http::Request<Incoming>> for RequestManager {
    type Response = http::Response<BoxBody<Bytes, Infallible>>;
    type Error = anyhow::Error;
    type Future = BoxFuture<anyhow::Result<Self::Response>>;

    fn call(&self, req: http::Request<Incoming>) -> Self::Future {
        Box::pin(handle_request(req, self.remote_addr, "http"))
    }
}
