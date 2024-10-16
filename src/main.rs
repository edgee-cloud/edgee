use std::path::PathBuf;

use clap::Parser;
use tracing::error;

mod config;
mod logger;
mod monitor;
mod proxy;
mod server;
mod tools;

#[derive(Debug, Parser)]
#[command(about, author, version)]
struct Options {
    #[arg(long, env = "EDGEE_LOG_FORMAT", value_enum, default_value_t)]
    log_format: logger::LogFormat,

    #[arg(short = 'f', long = "config", env = "EDGEE_CONFIG_PATH")]
    config_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let options = Options::parse();

    config::config::init(options.config_path.as_deref());
    logger::init(options.log_format);
    proxy::compute::data_collection::components::init();

    tokio::select! {
        Err(err) = monitor::start() => error!(?err, "Monitor failed"),
        Err(err) = server::start() => error!(?err, "Server failed to start"),
    }
}
