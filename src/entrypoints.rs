use std::net::{SocketAddr, ToSocketAddrs};
use std::{fs, io, sync::Arc};

use anyhow::Result;
use hyper_util::rt::TokioIo;
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error};

use crate::config;
use crate::domains;

pub async fn start() -> Result<()> {
    let mut joinset = JoinSet::new();

    for cfg in &config::get().entrypoints {
        if cfg.tls {
            run(&mut joinset, cfg.clone()).await;
        } else {
            run_without_tls(&mut joinset, cfg.clone()).await;
        }
    }

    let Some(_) = joinset.join_next().await else {
        todo!();
    };

    Ok(())
}

async fn run_without_tls(joinset: &mut JoinSet<()>, cfg: config::EntryPointConfiguration) {
    debug!(
        name = cfg.name,
        binding = cfg.bind,
        tls = cfg.tls,
        "starting entrypoint"
    );
    let addr: SocketAddr = cfg
        .bind
        .to_socket_addrs()
        .unwrap()
        .next()
        .expect("Valid socket address");

    let listener = TcpListener::bind(addr).await.unwrap();
    joinset.spawn(async move {
        loop {
            let (stream, addr) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);
            domains::respond_without_tls(cfg.domains.clone(), io, addr);
        }
    });
}

async fn run(joinset: &mut JoinSet<()>, cfg: config::EntryPointConfiguration) {
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

    debug!(
        name = cfg.name,
        binding = cfg.bind,
        tls = cfg.tls,
        "starting entrypoint"
    );
    let addr: SocketAddr = cfg
        .bind
        .to_socket_addrs()
        .unwrap()
        .next()
        .expect("Valid socket address");
    let listener = TcpListener::bind(addr).await.unwrap();

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

    joinset.spawn(async move {
        loop {
            let (stream, addr) = listener.accept().await.unwrap();
            let tls_acceptor = tls_acceptor.clone();
            let tls_stream = match tls_acceptor.accept(stream).await {
                Ok(tls_socket) => tls_socket,
                Err(err) => {
                    error!(?err, "Failed to perform tls handshake");
                    return;
                }
            };
            let io = TokioIo::new(tls_stream);
            domains::respond(cfg.domains.clone(), io, addr);
        }
    });
}
