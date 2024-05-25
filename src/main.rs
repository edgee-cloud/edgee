mod config;
mod logger;
mod providers;
mod proxy;

use std::sync::Arc;

use clap::Parser;

use config::Config;
use providers::Provider;
use tracing::error;

#[derive(Parser, Debug)]
#[command(about)]
pub struct Args {
    #[arg(short, long)]
    pub config_file: String, // path to the configuration file
}

pub struct Platform {
    pub provider: Provider,
    pub config: Config,
}

#[tokio::main]
async fn main() {
    let arg = Args::parse();

    let cfg = config::parse(arg.config_file);

    logger::init(&cfg.log.level);

    let platform = Arc::new(Platform {
        provider: providers::load(&cfg.providers),
        config: cfg,
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
