use std::{fs, io, net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http::{header::HOST, Request, Response, StatusCode, Uri};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error};

use crate::config;

pub async fn start() -> anyhow::Result<()> {
    let cfg = config::get();

    tokio::select! {
        Err(err) = start_http(cfg) => {
            error!(?err, "Failed to start HTTPS entrypoint");
            Err(err)
        }
        Err(err) = start_https(cfg) => {
            error!(?err, "Failed to start HTTPS entrypoint");
            Err(err)
        }
    }
}

async fn start_http(cfg: &config::StaticConfiguration) -> anyhow::Result<()> {
    debug!(
        address = cfg.http.address,
        force_https = cfg.http.force_https,
        "Starting HTTP entrypoint"
    );
    let addr: SocketAddr = cfg.http.address.parse()?;
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(a) => a,
            Err(err) => {
                error!(?err, ?addr, "Failed to listen for connections");
                continue;
            }
        };

        let cfg = cfg.clone();
        tokio::spawn(async move {
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(
                    TokioIo::new(stream),
                    service_fn(|req: Request<Incoming>| async move {
                        if cfg.http.force_https {
                            force_https(req).await
                        } else {
                            handle_request(req).await
                        }
                    }),
                )
                .await
            {
                error!(?err, ?addr, "Failed to serve connections");
            }
        });
    }
}

async fn start_https(cfg: &config::StaticConfiguration) -> anyhow::Result<()> {
    debug!(address = cfg.https.address, "Starting HTTPS entrypoint");
    let addr: SocketAddr = cfg.https.address.parse()?;
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
    let certs = load_certs(&cfg.https.cert).unwrap();
    let key = load_key(&cfg.https.key).unwrap();
    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];
    let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

    loop {
        let (stream, addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();
        tokio::spawn(async move {
            let tls_stream = match tls_acceptor.accept(stream).await {
                Ok(tls_stream) => tls_stream,
                Err(err) => {
                    error!(?err, "kfailed to perform tls handshake");
                    return;
                }
            };
            let io = TokioIo::new(tls_stream);
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(io, service_fn(handle_request))
                .await
            {
                error!(?err, ?addr, "Failed to serve connections");
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

async fn handle_request(req: Request<Incoming>) -> Resp {
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
