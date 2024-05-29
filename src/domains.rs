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
use tokio_rustls::server::TlsStream;
use tracing::error;

use crate::config;

pub fn respond(
    _cfg: Vec<config::DomainConfiguration>,
    io: TokioIo<TlsStream<TcpStream>>,
    _addr: SocketAddr,
) {
    tokio::spawn(async move {
        let _ = Builder::new(TokioExecutor::new())
            .serve_connection(io, service_fn(handle_request))
            .await
            .map_err(|err| error!(?err, "Failed to serve connection"));
    });
}

pub fn respond_without_tls(
    _cfg: Vec<config::DomainConfiguration>,
    io: TokioIo<TcpStream>,
    _addr: SocketAddr,
) {
    tokio::spawn(async move {
        let _ = Builder::new(TokioExecutor::new())
            .serve_connection(io, service_fn(handle_request))
            .await
            .map_err(|err| error!(?err, "Failed to serve connection"));
    });
}

enum Action {
    ShowStatus,
    ForceHttps(String),
    CustomEvent(String, Incoming),
    ForwardHttp {
        method: Method,
        host: String,
        headers: HeaderMap,
        parts: Parts,
        body: Incoming,
    },
}

async fn handle_request(req: Request<Incoming>) -> ProxyResult {
    debug!(method=%req.method(), uri=%req.uri(), "Request");
    let is_http = req.is_http();
    let force_https = platform.config.force_https;

    match parse_request(req, &platform.config) {
        Action::ShowStatus => {
            debug!("Show status");
            Ok(Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(empty())
                .unwrap())
        }
        Action::ForceHttps(uri) => {
            debug!(is_http, force_https, "Forcing https");
            Ok(Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header("location", uri)
                .body(empty())
                .unwrap())
        }
        Action::CustomEvent(_path, _body) => todo!(),
        Action::ForwardHttp {
            method,
            host,
            headers,
            parts,
            body,
        } => match platform.provider.get(host) {
            Some(endpoint) => {
                debug!("Matched endpont: {}", endpoint.hostname);
                let backend = endpoint.get_backend(parts.uri.to_string()).unwrap();

                debug!("Forwarding request to: {}", backend.location);
                let (host, port) = parse_host(backend.location.as_str());
                let addr = format!("{}:{}", host, port);

                let host = backend.location.as_str();
                let mut req = Request::builder().uri(&addr).method(method);
                let new_headers = req.headers_mut().unwrap();

                for (name, value) in headers.iter() {
                    new_headers.insert(name, value.to_owned());
                }

                new_headers.insert(HOST, HeaderValue::from_str(host).unwrap());

                let req = req.body(body).unwrap();

                debug!("Connecting to: {}", addr);
                let client = Client::builder(TokioExecutor::new()).build(HttpConnector::new());
                let res = client.request(req).await.unwrap().map(|r| r.boxed());

                Ok(res)
            }
            None => {
                let res = Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(empty())
                    .unwrap();
                Ok(res)
            }
        },
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
        Some(part) => part.parse::<u16>().unwrap_or(80),
        None => 80,
    };

    (host, port)
}

fn parse_request(req: Request<Incoming>, config: &Config) -> Action {
    if is_status_request(&req) {
        return Action::ShowStatus;
    }

    if let Some(uri) = forced_https_uri(&req, config.force_https) {
        return Action::ForceHttps(uri);
    }

    let is_http = req.is_http();
    let uri = req.uri().clone();
    let method = req.method().clone();
    let headers = req.headers().clone();

    let (parts, body) = req.into_parts();
    let host = extract_host(&parts.headers, &parts.uri).unwrap();

    if let Some(path) = custom_event(method.clone(), uri.clone()) {
        return Action::CustomEvent(path, body);
    }

    if is_http {
        Action::ForwardHttp {
            method,
            host,
            headers,
            parts,
            body,
        }
    } else {
        Action::ForwardHttps {
            method,
            host,
            headers,
            parts,
            body,
        }
    }
}

fn is_status_request(req: &Request<Incoming>) -> bool {
    let pq = req.uri().clone().into_parts().path_and_query.unwrap();
    req.method() == Method::GET && pq.path() == "/_edgee/status"
}

fn forced_https_uri(req: &Request<Incoming>, force_https: bool) -> Option<String> {
    if req.is_http() && force_https {
        let mut uri_parts = req.uri().clone().into_parts();
        uri_parts.scheme = Some("https".parse().unwrap());
        Uri::from_parts(uri_parts).unwrap();
        Some(req.https_uri().to_string())
    } else {
        None
    }
}

fn custom_event(method: Method, uri: Uri) -> Option<String> {
    if method != Method::POST {
        return None;
    }

    let pq = uri.into_parts().path_and_query.unwrap();
    Some(pq.path().to_string())
}
