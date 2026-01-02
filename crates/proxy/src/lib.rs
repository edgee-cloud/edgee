use std::sync::Arc;
use std::{convert::Infallible, fs, io, net::SocketAddr};

use bytes::Bytes;
use edgee_components_runtime::context::ComponentsContext;
use http_body_util::combinators::BoxBody;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use hyper_util::service::TowerToHyperService;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio::sync::OnceCell;
use tokio_rustls::TlsAcceptor;
use tower::util::BoxCloneService;
use tower_http::compression::CompressionBody;
use tracing::{error, info};

pub mod config;
pub mod monitor;
mod proxy;
mod tools;

type Body = CompressionBody<BoxBody<Bytes, Infallible>>;
static COMPONENTS_CONTEXT: OnceCell<ComponentsContext> = OnceCell::const_new();

pub fn init() -> anyhow::Result<()> {
    let components_configuration = &config::get().components;
    let ctx = ComponentsContext::new(components_configuration)?;

    COMPONENTS_CONTEXT
        .set(ctx)
        .map_err(|err| anyhow::anyhow!("Failed to register ComponentsContext: {err}"))
}

pub fn get_components_ctx() -> &'static ComponentsContext {
    COMPONENTS_CONTEXT
        .get()
        .expect("ComponentsContext should be registered")
}

pub async fn start() -> anyhow::Result<()> {
    use futures::future::try_join_all;

    let config = config::get();
    let mut tasks = Vec::new();

    if config.http.is_some() {
        tasks.push(tokio::spawn(async {
            if let Err(err) = http().await {
                anyhow::bail!("Failed to start HTTP entrypoint: {err}");
            }

            Ok(())
        }));
    }

    if config.https.is_some() {
        tasks.push(tokio::spawn(async {
            if let Err(err) = https().await {
                anyhow::bail!("Failed to start HTTPS entrypoint: {err}");
            }

            Ok(())
        }));
    }

    try_join_all(tasks)
        .await?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map(|_| ())
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

            let service = TowerToHyperService::new(make_service(remote_addr, "https"));

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
) -> BoxCloneService<proxy::Request, http::Response<Body>, anyhow::Error> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::init_test_config;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_init() {
        init_test_config();
        init().unwrap();
        let ctx = get_components_ctx();
        assert_eq!(ctx.engine.is_async(), true);
    }
}
