use std::net::{SocketAddr, ToSocketAddrs};

use anyhow::Result;
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tracing::debug;

use crate::config;
use crate::domains;

pub async fn start() -> Result<()> {
    let mut joinset = JoinSet::new();

    for cfg in &config::get().entrypoints {
        debug!(name = cfg.name, binding = cfg.bind, "starting entrypoint");
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
                domains::respond(&cfg.domains, stream, addr);
            }
        });
    }

    let Some(result) = joinset.join_next().await else {
        todo!();
    };

    result?
}
