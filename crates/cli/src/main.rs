use std::path::PathBuf;

use clap::Parser;
use tracing::error;

mod config;
mod logger;

#[derive(Debug, Parser)]
#[command(about, author, version)]
struct Options {
    #[arg(long, env = "EDGEE_LOG_FORMAT", value_enum, default_value_t)]
    log_format: logger::LogFormat,

    #[arg(short = 'f', long = "config", env = "EDGEE_CONFIG_PATH")]
    config_path: Option<PathBuf>,

    #[arg(
        short = 'c',
        long = "debug-component",
        help = "Launch Edgee and log only the specified component requests and responses to debug.",
        id = "COMPONENT_NAME"
    )]
    debug_component: Option<String>,
}

#[tokio::main]
async fn main() {
    let options = Options::parse();

    config::init(&options);
    // if debug_component is set, we only want to log the specified component. We change the options.log_format to do it.
    let mut log_filter = None;
    if options.debug_component.is_some() {
        // We disable all logs because component will print things to stdout directly
        log_filter = Some("off".to_string());
    }

    logger::init(options.log_format, log_filter);

    edgee_server::init();

    tokio::select! {
        Err(err) = edgee_server::monitor::start() => error!(?err, "Monitor failed"),
        Err(err) = edgee_server::start() => error!(?err, "Server failed to start"),
    }
}
