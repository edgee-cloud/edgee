use std::sync::Arc;
use std::{fs, io, net::SocketAddr};

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use hyper_util::service::TowerToHyperService;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{error, info};

use crate::config::config::{self, HttpConfiguration, HttpsConfiguration};
use crate::proxy::proxy;

type Response = http::Response<tower_http::compression::CompressionBody<proxy::ResponseBody>>;

pub async fn start() -> anyhow::Result<()> {
    use futures::future::try_join_all;

    let config = config::get();
    let mut tasks = Vec::new();

    if let Some(ref config) = config.http {
        tasks.push(tokio::spawn(async {
            if let Err(err) = http(config).await {
                error!(?err, "Failed to start HTTP entrypoint");
            }
        }));
    }

    if let Some(ref config) = config.https {
        tasks.push(tokio::spawn(async {
            if let Err(err) = https(config).await {
                error!(?err, "Failed to start HTTPS entrypoint");
            }
        }));
    }

    let _ = try_join_all(tasks).await;
    Ok(())
}

async fn http(config: &HttpConfiguration) -> anyhow::Result<()> {
    info!(
        address = config.address,
        force_https = config.force_https,
        "Starting HTTP entrypoint"
    );

    let addr: SocketAddr = config.address.parse()?;
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, remote_addr) = listener.accept().await?;

        let io = TokioIo::new(stream);
        let service = TowerToHyperService::new(make_service(remote_addr, "http"));

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

async fn https(config: &HttpsConfiguration) -> anyhow::Result<()> {
    info!(address = config.address, "Starting HTTPS entrypoint");

    let addr: SocketAddr = config.address.parse()?;
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
    let certs = load_certs(&config.cert).unwrap();
    let key = load_key(&config.key).unwrap();
    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];
    let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();

        let tls_stream = match tls_acceptor.accept(stream).await {
            Ok(tls_stream) => tls_stream,
            Err(err) => {
                error!(?err, "failed to perform tls handshake");
                continue;
            }
        };

        let io = TokioIo::new(tls_stream);
        let service = TowerToHyperService::new(make_service(remote_addr, "https"));

        tokio::spawn(async move {
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(io, service)
                .await
            {
                error!(?err, "failed to serve connections");
            }
        });
    }
}

/// Create the service pipeline, using the proxy handler as the "final" request handler
///
/// Arguments:
/// - `remote_addr`: Remote client address
/// - `proto`: Protocol used (HTTP or HTTPS)
///
/// Returns:
///
/// A full service pipeline for handling a client request
fn make_service(
    remote_addr: SocketAddr,
    proto: &str,
) -> tower::util::BoxCloneService<proxy::Request, Response, anyhow::Error> {
    use tower::{ServiceBuilder, ServiceExt};
    use tower_http::compression::CompressionLayer;

    let proto = proto.to_string();
    ServiceBuilder::new()
        .layer(CompressionLayer::new())
        .service_fn(move |req| {
            let proto = proto.clone();
            async move { proxy::handle_request(req, remote_addr, &proto).await }
        })
        .boxed_clone()
}
