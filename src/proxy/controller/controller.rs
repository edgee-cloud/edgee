use std::convert::Infallible;
use std::net::SocketAddr;
use bytes::{Buf, Bytes};
use http::uri::PathAndQuery;
use http_body_util::combinators::BoxBody;
use http::{header, HeaderMap, StatusCode};
use http_body_util::{Empty, Full};
use http_body_util::BodyExt;
use log::warn;
use crate::proxy::compute::html;
use crate::proxy::compute::compute;
use crate::proxy::context::incoming::IncomingContext;

type Response = http::Response<BoxBody<Bytes, Infallible>>;

pub async fn edgee_client_event(incoming_ctx: IncomingContext, host: &String, path: &PathAndQuery, request_headers: &HeaderMap, remote_addr: &SocketAddr) -> anyhow::Result<Response> {
    let mut res = http::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(empty())?;

    let response_headers = res.headers_mut();
    let body = incoming_ctx.incoming_body.collect().await?.to_bytes();
    if let Ok(mut payload) = serde_json::from_reader(body.reader()) {
        compute::json_handler(&mut payload, host, path, request_headers, remote_addr, response_headers).await;
    }
    Ok(res)
}

pub async fn edgee_client_event_from_third_party_sdk() -> anyhow::Result<Response> {
    warn!("edgee_client_event_from_third_party_sdk is not implemented");
    Ok(http::Response::builder()
        .status(StatusCode::NOT_IMPLEMENTED)
        .body(empty())
        .expect("response builder should never fail"))
}

pub fn options(allow_methods: &str) -> anyhow::Result<Response> {
    Ok(http::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_METHODS, allow_methods)
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(header::ACCESS_CONTROL_MAX_AGE, "3600")
        .body(empty())
        .expect("response builder should never fail"))
}

pub fn redirect_to_https(incoming_host: String, incoming_path: PathAndQuery) -> anyhow::Result<Response> {
    Ok(http::Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(header::LOCATION, format!("https://{}{}", incoming_host, incoming_path))
        .header(header::CONTENT_TYPE, "text/plain")
        .body(empty())
        .expect("response builder should never fail"))
}

pub fn sdk(path: &str) -> anyhow::Result<Response> {
    let inlined_sdk = html::get_sdk_from_url(path);
    if inlined_sdk.is_ok() {
        Ok(http::Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
            .header(header::CACHE_CONTROL, "public, max-age=300")
            .body(Full::from(Bytes::from(inlined_sdk.unwrap())).boxed())
            .expect("serving sdk should never fail"))
    } else {
        Ok(http::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CACHE_CONTROL, "public, max-age=300")
            .body(Full::from(Bytes::from("Not found")).boxed())
            .expect("serving sdk should never fail"))
    }
}

pub fn bad_gateway_error() -> anyhow::Result<Response> {
    static HTML: &str = include_str!("../../../public/502.html");
    Ok(http::Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Full::from(Bytes::from(HTML)).boxed())
        .expect("response builder should never fail"))
}

fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}
