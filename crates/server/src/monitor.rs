use crate::config;
use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use std::{convert::Infallible, net::SocketAddr};
use tokio::net::TcpListener;
use tracing::{debug, error, info};

pub async fn start() -> anyhow::Result<()> {
    match &config::get().monitor {
        Some(cfg) => {
            info!(address = cfg.address, "Starting monitor");
            let addr: SocketAddr = cfg.address.parse()?;
            let listener = TcpListener::bind(addr)
                .await
                .expect("Should bind to socket addr");

            loop {
                let (stream, addr) = match listener.accept().await {
                    Ok(a) => a,
                    Err(err) => {
                        error!(?err, "Failed to accepts connection");
                        continue;
                    }
                };

                if let Err(err) = Builder::new(TokioExecutor::new())
                    .serve_connection(TokioIo::new(stream), service_fn(handle_request))
                    .await
                {
                    error!(?err, ?addr, "Failed to serve connections");
                }
            }
        }
        None => Ok(()),
    }
}

async fn handle_request(
    req: Request<Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, Infallible> {
    match req.uri().path() {
        "/healthz" => {
            let res = Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(
                    Empty::<Bytes>::new()
                        .map_err(|never| match never {})
                        .boxed(),
                )
                .expect("Should build body");
            debug!("monitor responded ok");
            Ok(res)
        }
        _ => {
            let res = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(
                    Empty::<Bytes>::new()
                        .map_err(|never| match never {})
                        .boxed(),
                )
                .expect("Should build body");
            debug!("monitor responded not found");
            Ok(res)
        }
    }
}
