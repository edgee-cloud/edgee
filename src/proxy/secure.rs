use anyhow::Result;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::{fs, io, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{error, info};

use crate::Platform;

pub async fn start(platform: Arc<Platform>) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], platform.config.https_port));
    let listener = TcpListener::bind(addr).await?;
    info!(port = addr.port(), "HTTPS");

    let _ = rustls::crypto::ring::default_provider().install_default();
    let certs = load_certs("local/server.pem").unwrap();
    let key = load_private_key("local/server.key").unwrap();
    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| error!(?err))
        .unwrap();
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];
    let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

    loop {
        let (socket, _) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();
        let p = platform.clone();
        tokio::spawn(async move {
            let tls_socket = match tls_acceptor.accept(socket).await {
                Ok(tls_socket) => tls_socket,
                Err(err) => {
                    error!(?err, "Failed to perform tls handshake");
                    return;
                }
            };
            let io = TokioIo::new(tls_socket);
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(|req| super::handle_request(p.clone(), req)))
                .await
            {
                error!(?err, "Failed to handle request");
            }
        });
    }
}

fn load_certs(filename: &str) -> io::Result<Vec<CertificateDer<'static>>> {
    let certfile = fs::File::open(filename)
        .map_err(|err| error!(?err, filename, "Failed to open certificate"))
        .unwrap();
    let mut reader = io::BufReader::new(certfile);
    rustls_pemfile::certs(&mut reader).collect()
}

fn load_private_key(filename: &str) -> io::Result<PrivateKeyDer<'static>> {
    let keyfile = fs::File::open(filename)
        .map_err(|err| error!(?err, filename, "Filed to open private key"))
        .unwrap();
    let mut reader = io::BufReader::new(keyfile);
    rustls_pemfile::private_key(&mut reader).map(|key| key.unwrap())
}
