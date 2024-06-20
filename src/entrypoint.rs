use std::{convert::Infallible, io::Read, net::SocketAddr, str::FromStr};

use anyhow::bail;
use bytes::Bytes;
use http::{
    header::{CONTENT_ENCODING, HOST},
    uri::PathAndQuery,
    HeaderValue, StatusCode, Uri,
};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::body::Incoming;
use hyper_rustls::ConfigBuilderExt;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use libflate::{deflate, gzip};
use regex::Regex;
use tracing::{debug, error};

use crate::config;
use crate::html;

pub async fn start() -> anyhow::Result<()> {
    tokio::select! {
        Err(err) = web::start() => {
            error!(?err, "Failed to start HTTPS entrypoint");
            Err(err)
        }
        Err(err) = websecure::start() => {
            error!(?err, "Failed to start HTTPS entrypoint");
            Err(err)
        }
    }
}

type Response = http::Response<BoxBody<Bytes, Infallible>>;

mod web {
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
            .header("localtion", uri)
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
}

mod websecure {
    use std::{convert::Infallible, fs, future::Future, io, net::SocketAddr, pin::Pin, sync::Arc};

    use bytes::Bytes;
    use http_body_util::combinators::BoxBody;
    use hyper::body::Incoming;
    use hyper_util::{
        rt::{TokioExecutor, TokioIo},
        server::conn::auto::Builder,
    };
    use rustls::ServerConfig;
    use rustls_pki_types::{CertificateDer, PrivateKeyDer};
    use tokio::net::TcpListener;
    use tokio_rustls::TlsAcceptor;
    use tracing::{error, info};

    use crate::config;

    use super::handle_request;

    pub async fn start() -> anyhow::Result<()> {
        let cfg = &config::get().https;

        info!(address = cfg.address, "Starting HTTPS entrypoint");
        let addr: SocketAddr = cfg.address.parse()?;
        let listener = TcpListener::bind(addr).await?;

        fn load_certs(filename: &str) -> io::Result<Vec<CertificateDer<'static>>> {
            let certfile = fs::File::open(filename).unwrap();
            let mut reader = io::BufReader::new(certfile);
            rustls_pemfile::certs(&mut reader).collect()
        }

        fn load_key(filename: &str) -> io::Result<PrivateKeyDer<'static>> {
            let keyfile = fs::File::open(filename).unwrap();
            let mut reader = io::BufReader::new(keyfile);
            rustls_pemfile::private_key(&mut reader).map(|key| key.unwrap())
        }

        let _ = rustls::crypto::ring::default_provider().install_default();
        let certs = load_certs(&cfg.cert).unwrap();
        let key = load_key(&cfg.key).unwrap();
        let mut server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap();
        server_config.alpn_protocols =
            vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];
        let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

        loop {
            let (stream, remote_addr) = listener.accept().await?;
            let tls_acceptor = tls_acceptor.clone();
            tokio::spawn(async move {
                let tls_stream = match tls_acceptor.accept(stream).await {
                    Ok(tls_stream) => tls_stream,
                    Err(err) => {
                        error!(?err, "failed to perform tls handshake");
                        return;
                    }
                };
                let io = TokioIo::new(tls_stream);
                let service = RequestManager::new(remote_addr);
                if let Err(err) = Builder::new(TokioExecutor::new())
                    .serve_connection_with_upgrades(io, service)
                    .await
                {
                    error!(?err, "failed to serve connections");
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
            Box::pin(handle_request(req, self.remote_addr, "https"))
        }
    }
}

async fn handle_request(
    mut req: http::Request<Incoming>,
    remote_addr: SocketAddr,
    proto: &str,
) -> anyhow::Result<Response> {
    let host = match (req.headers().get(HOST), req.uri().host()) {
        (None, Some(value)) => Some(String::from(value)),
        (Some(value), _) => Some(value.to_str().unwrap().to_string()),
        (None, None) => None,
    }
    .and_then(|host| host.split(':').next().map(|s| s.to_string()))
    .expect("host should be available");

    let cfg = &config::get().routing;
    let routing = cfg.iter().find(|r| r.domain == host);

    if routing.is_none() {
        return Ok(error_bad_gateway());
    };

    const FORWARDED_FOR: &str = "x-forwarded-for";
    let client_ip = remote_addr.ip().to_string();
    if let Some(forwarded_for) = req.headers_mut().get_mut(FORWARDED_FOR) {
        let existing_value = forwarded_for.to_str().unwrap();
        let new_value = format!("{}, {}", existing_value, client_ip);
        *forwarded_for = HeaderValue::from_str(&new_value).expect("header value should be valid");
    } else {
        req.headers_mut().insert(
            FORWARDED_FOR,
            HeaderValue::from_str(&client_ip).expect("header value should be valid"),
        );
    }

    const FORWARDED_PROTO: &str = "x-forwarded-proto";
    if req.headers().get(FORWARDED_PROTO).is_none() {
        req.headers_mut().insert(
            FORWARDED_PROTO,
            HeaderValue::from_str(proto).expect("header value should be valid"),
        );
    }

    const FORWARDED_HOST: &str = "x-forwarded-host";
    if req.headers().get(FORWARDED_HOST).is_none() {
        req.headers_mut().insert(
            FORWARDED_HOST,
            HeaderValue::from_str(&host).expect("header value should be valid"),
        );
    }

    let routing = routing.expect("router should have some value");

    let default_backend = match routing.backends.iter().find(|b| b.default) {
        Some(a) => a,
        None => {
            return Ok(error_bad_gateway());
        }
    };

    let uri = req.uri_mut();
    let root_path = PathAndQuery::from_str("/").expect("'/' should be a valid path");
    let requested_path = uri.path_and_query().unwrap_or(&root_path);

    let mut upstream_backend: Option<&config::BackendConfiguration> = None;
    let mut upstream_path: Option<PathAndQuery> = None;
    for rule in routing.rules.clone() {
        match (rule.path, rule.path_prefix, rule.path_regexp) {
            (Some(path), _, _) => {
                if *requested_path == *path {
                    upstream_backend = match rule.backend {
                        Some(name) => routing.backends.iter().find(|b| b.name == name),
                        None => Some(default_backend),
                    };
                    upstream_path = match rule.rewrite {
                        Some(replacement) => PathAndQuery::from_str(&replacement).ok(),
                        None => PathAndQuery::from_str(&path).ok(),
                    };
                    break;
                }
            }
            (None, Some(prefix), _) => {
                if requested_path.to_string().starts_with(&prefix) {
                    upstream_backend = match rule.backend {
                        Some(name) => routing.backends.iter().find(|b| b.name == name),
                        None => Some(default_backend),
                    };
                    upstream_path = match rule.rewrite {
                        Some(replacement) => {
                            let new_path =
                                requested_path
                                    .to_string()
                                    .replacen(&prefix, &replacement, 1);
                            PathAndQuery::from_str(&new_path).ok()
                        }
                        None => Some(requested_path.clone()),
                    };
                    break;
                }
            }
            (None, None, Some(pattern)) => {
                let regexp = Regex::new(&pattern).expect("regex pattern should be valid");
                let path = requested_path.to_string();
                if regexp.is_match(&path) {
                    upstream_backend = match rule.backend {
                        Some(name) => routing.backends.iter().find(|b| b.name == name),
                        None => Some(default_backend),
                    };
                    upstream_path = match rule.rewrite {
                        Some(replacement) => {
                            PathAndQuery::from_str(&regexp.replacen(&path, 1, &replacement)).ok()
                        }
                        None => PathAndQuery::from_str(&path).ok(),
                    };
                }
            }
            _ => bail!("Invalid routing"),
        }
    }

    let backend = upstream_backend.unwrap_or(default_backend);
    let path = upstream_path.unwrap_or(requested_path.clone());

    let res = if backend.enable_ssl {
        forward_https_request(req, backend, path).await
    } else {
        forward_http_request(req, backend, path).await
    };

    match res {
        Err(err) => {
            error!(?err, "backend request failed");
            Ok(error_bad_gateway())
        }
        Ok(upstream) => {
            let headers = upstream.headers().clone();
            let encoding = headers.get(CONTENT_ENCODING).and_then(|h| h.to_str().ok());
            let body = upstream.collect().await?.to_bytes();
            let cursor = std::io::Cursor::new(body.clone());
            let decompressed_body = match encoding {
                Some("gzip") => {
                    let mut decoder = gzip::Decoder::new(cursor)?;
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf)?;
                    String::from_utf8_lossy(&buf).to_string()
                }
                Some("deflate") => {
                    let mut decoder = deflate::Decoder::new(cursor);
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf)?;
                    String::from_utf8_lossy(&buf).to_string()
                }
                Some("brotli") => {
                    let mut decoder = brotli::Decompressor::new(cursor, 4096);
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf)?;
                    String::from_utf8_lossy(&buf).to_string()
                }
                Some(_) | None => String::from_utf8_lossy(&body).to_string(),
            };

            let new_body = match parse_body(&decompressed_body) {
                Embedding::Empty => decompressed_body,
                Embedding::Doc(_) => decompressed_body,
            };

            Ok(Response::new(full(new_body)))
        }
    }
}

enum Embedding {
    Empty,
    Doc(html::Document),
}

fn parse_body(body: &str) -> Embedding {
    if !body.contains(r#"id="__EDGEE_SDK__""#) {
        return Embedding::Empty;
    }

    Embedding::Doc(html::parse_html(body))
}

async fn forward_http_request(
    orig: http::Request<Incoming>,
    backend: &config::BackendConfiguration,
    path: PathAndQuery,
) -> anyhow::Result<http::Response<Incoming>> {
    let uri: Uri = format!("http://{}{}", &backend.address, path)
        .parse()
        .expect("uri should be valid");

    debug!(origin=?orig.uri(),?uri, "Forwarding HTTP request");

    let mut req = http::Request::builder().uri(uri).method(orig.method());
    let headers = req.headers_mut().expect("request should have headers");
    for (name, value) in orig.headers().iter() {
        headers.insert(name, value.to_owned());
    }

    headers.insert(
        "host",
        HeaderValue::from_str(&backend.address).expect("host should be valid"),
    );

    let (_parts, body) = orig.into_parts();
    let req = req.body(body).expect("request to be built");
    let client = Client::builder(TokioExecutor::new()).build(HttpConnector::new());
    client
        .request(req)
        .await
        .map_err(|err| anyhow::Error::new(err))
}

async fn forward_https_request(
    mut req: http::Request<Incoming>,
    backend: &config::BackendConfiguration,
    path: PathAndQuery,
) -> anyhow::Result<http::Response<Incoming>> {
    let uri: Uri = format!("https://{}{}", &backend.address, path)
        .parse()
        .expect("uri should be valid");

    *req.uri_mut() = uri;

    req.headers_mut().insert(
        "host",
        HeaderValue::from_str(&backend.address).expect("host should be valid"),
    );

    let client_config = rustls::ClientConfig::builder()
        .with_native_roots()?
        .with_no_client_auth();
    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(client_config)
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();
    let client = Client::builder(TokioExecutor::new()).build(connector);
    client
        .request(req)
        .await
        .map_err(|err| anyhow::Error::new(err))
}

fn error_bad_gateway() -> Response {
    static HTML: &str = include_str!("../public/502.html");
    http::Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Full::from(Bytes::from(HTML)).boxed())
        .expect("response builder should never fail")
}

fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}
