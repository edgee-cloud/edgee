use std::{convert::Infallible, net::ToSocketAddrs};

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use tokio::net::TcpListener;
use tracing::error;

use crate::config;

pub async fn start() -> anyhow::Result<()> {
    let cfg = &config::get().monitor;

    let addr = cfg
        .http
        .to_socket_addrs()
        .expect("Should convert string to socket addr")
        .next()
        .expect("Should have at least one socket addr");

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
            Ok(res)
        }
    }
}
