use std::{fs, io, net::SocketAddr, str::FromStr, sync::Arc};

use anyhow::bail;
use bytes::Bytes;
use http::{header::HOST, uri::PathAndQuery, HeaderValue, Request, Response, StatusCode, Uri};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::{body::Incoming, service::service_fn};
use hyper_rustls::ConfigBuilderExt;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use regex::Regex;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error};

use crate::config;

pub async fn start() -> anyhow::Result<()> {
    tokio::select! {
        Err(err) = start_http() => {
            error!(?err, "Failed to start HTTPS entrypoint");
            Err(err)
        }
        Err(err) = start_https() => {
            error!(?err, "Failed to start HTTPS entrypoint");
            Err(err)
        }
    }
}

async fn start_http() -> anyhow::Result<()> {
    let cfg = &config::get().http;

    debug!(
        address = cfg.address,
        force_https = cfg.force_https,
        "Starting HTTP entrypoint"
    );

    let addr: SocketAddr = cfg.address.parse()?;
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, remote_addr) = match listener.accept().await {
            Ok(a) => a,
            Err(err) => {
                error!(?err, "Failed to listen for connections");
                continue;
            }
        };

        let cfg = cfg.clone();
        tokio::spawn(async move {
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(
                    TokioIo::new(stream),
                    service_fn(|req: Request<Incoming>| async move {
                        if cfg.force_https {
                            force_https(req).await
                        } else {
                            handle_request(req, remote_addr, "http").await
                        }
                    }),
                )
                .await
            {
                error!(?err, ?remote_addr, "Failed to serve connections");
            }
        });
    }
}

async fn start_https() -> anyhow::Result<()> {
    let cfg = &config::get().https;

    debug!(address = cfg.address, "Starting HTTPS entrypoint");
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
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];
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
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(
                    io,
                    service_fn(|req| handle_request(req, remote_addr, "https")),
                )
                .await
            {
                error!(?err, "failed to serve connections");
            }
        });
    }
}

type Resp = anyhow::Result<Response<BoxBody<Bytes, hyper::Error>>>;

async fn force_https(req: Request<Incoming>) -> Resp {
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

    Ok(Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header("localtion", uri)
        .body(empty())
        .expect("body should never fail"))
}

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

async fn handle_request(mut req: Request<Incoming>, remote_addr: SocketAddr, proto: &str) -> Resp {
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
        return Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(empty())
            .expect("response should never fail"));
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
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(empty())
                .expect("response should never fail"));
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
    let path = upstream_path.unwrap_or(root_path);

    if proto == "http" {
        forward_http_request(req, backend, path).await
    } else {
        forward_https_request(req, backend, path).await
    }
}

async fn forward_http_request(
    orig: Request<Incoming>,
    backend: &config::BackendConfiguration,
    path: PathAndQuery,
) -> Resp {
    let uri: Uri = format!("http://{}{}", &backend.address, path)
        .parse()
        .expect("uri should be valid");

    let mut req = Request::builder().uri(uri).method(orig.method());
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
    match client.request(req).await {
        Ok(res) => Ok(res.map(|r| r.boxed())),
        Err(err) => {
            error!(?err, "failed to send request to backend");
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(empty())
                .expect("response should never fail"))
        }
    }
}

async fn forward_https_request(
    mut req: Request<Incoming>,
    backend: &config::BackendConfiguration,
    path: PathAndQuery,
) -> Resp {
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
    match client.request(req).await {
        Ok(res) => Ok(res.map(|r| r.boxed())),
        Err(err) => {
            error!(?err, "failed to send request to backend");
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(empty())
                .expect("response should never fail"))
        }
    }
}
