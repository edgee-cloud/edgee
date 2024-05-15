use std::{fs, io, net::SocketAddr, sync::Arc};

use anyhow::Result;
use bytes::Bytes;
use http::{Request, Response};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::error;

use crate::Platform;

pub async fn start(port: u16, _platform: &Platform) -> Result<()> {
    // set a process wide default crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
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
        tokio::spawn(async move {
            let tls_socket = match tls_acceptor.accept(socket).await {
                Ok(tls_socket) => tls_socket,
                Err(err) => {
                    error!(?err, "Failed to perform tls handshake");
                    return;
                }
            };
            if let Err(err) = Builder::new(TokioExecutor::new())
                .serve_connection(TokioIo::new(tls_socket), service_fn(hello))
                .await
            {
                error!(?err, "Failed to handle request");
            }
        });
    }
}

async fn hello(_req: Request<Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
    Ok(Response::new(Full::from("Hello")))
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
