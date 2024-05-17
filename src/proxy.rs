use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use http::{header::HOST, HeaderMap, HeaderValue, Request, Response, StatusCode, Uri};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::body::Incoming;
use hyper_util::client::legacy::Client;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};
use tracing::{debug, error};

use crate::{EventStream, Platform};

pub mod cleartext;
pub mod secure;

type ProxyResult = Result<Response<BoxBody<Bytes, hyper::Error>>>;

async fn handle_request(platform: Arc<Platform>, req: Request<Incoming>) -> ProxyResult {
    debug!(method=%req.method(), uri=%req.uri(), "Request");

    let method = req.method().clone();
    let original_headers = req.headers().clone();
    let (parts, body) = req.into_parts();

    let host = match extract_host(&parts.headers, &parts.uri) {
        Some(host) => host,
        None => {
            error!("Could not extract hostname");
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(empty())
                .unwrap());
        }
    };

    match platform.provider.get(host) {
        Some(endpoint) => {
            debug!("Matched endpont: {}", endpoint.hostname);
            let backend = endpoint.get_backend(parts.uri.to_string()).unwrap();

            debug!("Forwarding request to: {}", backend.location);
            let (host, port) = parse_host(backend.location.as_str());
            let addr = format!("{}:{}", host, port);

            let host = backend.location.as_str();
            let mut req = Request::builder().uri(&addr).method(method);
            let new_headers = req.headers_mut().unwrap();

            for (name, value) in original_headers.iter() {
                new_headers.insert(name, value.to_owned());
            }

            new_headers.insert(HOST, HeaderValue::from_str(host).unwrap());

            let req = req.body(body).unwrap();
            let uri = req.uri().to_string();

            debug!("Connecting to: {}", addr);
            let client = Client::builder(TokioExecutor::new()).build(HttpConnector::new());
            let res = client.request(req).await.unwrap().map(|r| r.boxed());

            platform
                .sender
                .send(EventStream::PageView(uri))
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

fn extract_host(header: &HeaderMap<HeaderValue>, uri: &Uri) -> Option<String> {
    match (header.get(HOST), uri.host()) {
        (Some(value), _) => Some(value.to_str().unwrap().to_string()),
        (None, Some(value)) => Some(String::from(value)),
        (None, None) => None,
    }
}

fn parse_host(host: &str) -> (String, u16) {
    let parts: Vec<&str> = host.split(':').collect();
    let host = parts[0].to_string();
    let port = match parts.get(1) {
        Some(part) => part.parse::<u16>().unwrap_or(443),
        None => 443,
    };

    (host, port)
}
