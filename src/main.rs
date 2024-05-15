mod config;
mod logger;
mod providers;
mod proxy;

use std::sync::Arc;

use providers::Provider;
use tokio::sync::mpsc::Sender;
use tracing::{debug, error};

#[derive(Debug)]
pub enum EventStream {
    PageView(String),
}

pub struct Platform {
    pub provider: Arc<Provider>,
    pub tx: Sender<EventStream>,
}

#[tokio::main]
async fn main() {
    let cfg = config::parse();
    logger::init(&cfg.log_severity);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<EventStream>(1024);
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            debug!(?event, "Received event");
        }
    });

    let platform = Platform {
        provider: Arc::new(providers::load()),
        tx,
    };

    tokio::select! {
        Err(err) = proxy::cleartext::start(cfg.http_port, &platform) => {
            error!(?err, "HTTP server failed");
            std::process::exit(1);
        }

        Err(err) = proxy::secure::start(cfg.https_port, &platform) => {
            error!(?err, "HTTPS server failed");
            std::process::exit(1);
        }
    }
}
