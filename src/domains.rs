use std::net::SocketAddr;

use anyhow::Result;
use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use tokio::net::TcpStream;
use tracing::error;

use crate::config;

pub fn respond(_cfg: &[config::DomainConfiguration], stream: TcpStream, _addr: SocketAddr) {
    async fn handle_request(
        _req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>> {
        Ok(Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(
                Empty::<Bytes>::new()
                    .map_err(|never| match never {})
                    .boxed(),
            )
            .unwrap())
    }

    tokio::spawn(async move {
        let io = TokioIo::new(stream);
        Builder::new(TokioExecutor::new())
            .serve_connection(io, service_fn(handle_request))
            .await
            .map_err(|err| error!(?err, "Failed to serve connection"))
    });
}
