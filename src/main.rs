mod config;
mod logger;
mod providers;
mod proxy;

use std::sync::Arc;

use config::Config;
use providers::Provider;
use tokio::sync::mpsc::Sender;
use tracing::{debug, error};

#[derive(Debug)]
pub enum EventStream {
    PageView(String),
}

pub struct Platform {
    pub provider: Provider,
    pub sender: Sender<EventStream>,
    pub config: Config,
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

    let platform = Arc::new(Platform {
        provider: providers::load(),
        sender: tx,
        config: cfg.clone(),
    });

    tokio::select! {
        Err(err) = proxy::cleartext::start(platform.clone()) => {
            error!(?err, "HTTP server failed");
            std::process::exit(1);
        }

        Err(err) = proxy::secure::start(platform.clone()) => {
            error!(?err, "HTTPS server failed");
            std::process::exit(1);
        }
    }
}
