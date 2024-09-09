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
    // let cfg = &config::get().https;
    let cfg = match &config::get().https {
        Some(cfg) => cfg,
        None => {
            return Err(anyhow::anyhow!("HTTPS configuration is missing"));
        }
    };

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
