use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use bytes::Bytes;
use http::header::HOST;
use http::StatusCode;
use http::Uri;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;
use tracing::debug;
use tracing::error;
use tracing::info;

use crate::config;

use super::empty;
use super::handle_request;
use super::Response;

pub async fn start() -> anyhow::Result<()> {
    let cfg = &config::get().http;

    info!(
        address = cfg.address,
        force_https = cfg.force_https,
        "started"
    );

    let addr: SocketAddr = cfg.address.parse()?;
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let cfg = cfg.clone();
        let io = TokioIo::new(stream);
        let service = RequestManager::new(cfg.clone(), remote_addr);
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

async fn force_https(req: http::Request<Incoming>) -> anyhow::Result<Response> {
    // FIXME: Append https port to hostname (if not the default 443)
    let host = match (req.headers().get(HOST), req.uri().host()) {
        (None, Some(value)) => Some(String::from(value)),
        (Some(value), _) => Some(value.to_str().unwrap().to_string()),
        (None, None) => None,
    }
    .and_then(|host| host.split(':').next().map(|s| s.to_string()))
    .expect("host should be available");

    let mut uri_parts = req.uri().clone().into_parts();
    uri_parts.scheme = Some("https".parse().expect("should be valid scheme"));
    uri_parts.authority = Some(host.parse().expect("should be valid host"));
    debug!(?uri_parts, "Forcing HTTPS redirection");
    let uri = Uri::from_parts(uri_parts)
        .expect("should be valid uri")
        .to_string();

    Ok(http::Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header("location", uri)
        .body(empty())
        .expect("body should never fail"))
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

struct RequestManager {
    config: config::HttpConfiguration,
    remote_addr: SocketAddr,
}

impl RequestManager {
    fn new(cfg: config::HttpConfiguration, addr: SocketAddr) -> Self {
        Self {
            config: cfg,
            remote_addr: addr,
        }
    }
}

impl hyper::service::Service<http::Request<Incoming>> for RequestManager {
    type Response = http::Response<BoxBody<Bytes, Infallible>>;
    type Error = anyhow::Error;
    type Future = BoxFuture<anyhow::Result<Self::Response>>;

    fn call(&self, req: http::Request<Incoming>) -> Self::Future {
        if self.config.force_https {
            Box::pin(force_https(req))
        } else {
            Box::pin(handle_request(req, self.remote_addr, "http"))
        }
    }
}
