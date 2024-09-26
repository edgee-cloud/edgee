use crate::config::config;
use crate::proxy;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use proxy::proxy::handle_request;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::{convert::Infallible, fs, io, net::SocketAddr};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{error, info};

pub async fn start() -> anyhow::Result<()> {
    let config = config::get();
    let mut tasks = Vec::new();

    if config.http.is_some() {
        tasks.push(tokio::spawn(async {
            if let Err(err) = http().await {
                error!(?err, "Failed to start HTTP entrypoint");
            }
        }));
    }

    if config.https.is_some() {
        tasks.push(tokio::spawn(async {
            if let Err(err) = https().await {
                error!(?err, "Failed to start HTTPS entrypoint");
            }
        }));
    }

    tokio::select! {
        _ = tasks.pop().unwrap() => Ok(()),
    }
}

async fn http() -> anyhow::Result<()> {
    let cfg = config::get()
        .http
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("HTTP configuration is missing"))?;

    info!(
        address = cfg.address,
        force_https = cfg.force_https,
        "Starting HTTP entrypoint"
    );

    let addr: SocketAddr = cfg.address.parse()?;
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let service = RequestManager::new(remote_addr, "http".to_string());
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

async fn https() -> anyhow::Result<()> {
    let cfg = config::get()
        .https
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("HTTPS configuration is missing"))?;

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
            let service = RequestManager::new(remote_addr, "https".to_string());
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

pub struct RequestManager {
    remote_addr: SocketAddr,
    proto: String,
}

impl RequestManager {
    pub fn new(addr: SocketAddr, proto: String) -> Self {
        Self {
            remote_addr: addr,
            proto,
        }
    }
}

impl hyper::service::Service<http::Request<Incoming>> for RequestManager {
    type Response = http::Response<BoxBody<Bytes, Infallible>>;
    type Error = anyhow::Error;
    type Future = BoxFuture<anyhow::Result<Self::Response>>;

    fn call(&self, req: http::Request<Incoming>) -> Self::Future {
        if self.proto == "https" {
            Box::pin(handle_request(req, self.remote_addr, "https"))
        } else {
            Box::pin(handle_request(req, self.remote_addr, "http"))
        }
    }
}
