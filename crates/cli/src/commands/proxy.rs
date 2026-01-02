use std::path::PathBuf;

use crate::logger;

setup_command! {
    #[arg(long, env = "EDGEE_LOG_FORMAT", value_enum, default_value_t)]
    log_format: logger::LogFormat,

    #[arg(short, long = "config", env = "EDGEE_CONFIG_PATH")]
    config_path: Option<PathBuf>,

    /// Log only the specified component's requests and responses to debug.
    #[arg(short, long, id = "COMPONENT_NAME")]
    trace_component: Option<String>,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    use crate::config;

    config::init(opts.config_path.as_deref(), opts.trace_component.as_deref());
    // if trace_component is set, we only want to log the specified component. We change the options.log_format to do it.
    let mut log_filter = None;
    if opts.trace_component.is_some() {
        // We disable all logs because component will print things to stdout directly
        log_filter = Some("off".to_string());
    }

    logger::init(opts.log_format, log_filter);

    edgee_proxy::init()?;

    tokio::select! {
        Err(err) = edgee_proxy::monitor::start() => Err(err),
        Err(err) = edgee_proxy::start() => Err(err),
    }
}
