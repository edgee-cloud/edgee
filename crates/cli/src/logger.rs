/// Define log format used
///
/// Defaults to `Basic` when building in debug profile, `Json` when building in release profile
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum LogFormat {
    #[cfg_attr(debug_assertions, default)]
    Basic,
    Pretty,
    #[cfg_attr(not(debug_assertions), default)]
    Json,
}

/// Initializing logging facilities
///
/// Log filtering is configured with env var and config in this priority order:
/// - `log_filter` parameter
/// - `EDGEE_LOG` env variable, formatted for [tracing_subscriber::EnvFilter]
/// - `RUST_LOG` "standard" env variable, also formatted for [tracing_subscriber::EnvFilter]
/// - Config file, specifying level and optionally span
///
/// In the case something goes wrong with parsing of these directives, logging is done
/// using the log level defined in config
pub fn init(log_format: LogFormat, log_filter: Option<String>) {
    use std::env;

    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    let config = &edgee_server::config::get().log;

    let with_target = log_filter.is_none();
    let fmt_layer = fmt::layer().with_target(with_target);

    let fmt_layer = match log_format {
        LogFormat::Basic | LogFormat::Pretty => fmt_layer.boxed(),
        LogFormat::Json => fmt_layer.json().boxed(),
    };

    let filter_layer = {
        let builder = EnvFilter::builder().with_default_directive(config.level.into());

        // Get logging directives from log_filter or EDGEE_LOG or standard RUST_LOG env variables
        let directives = log_filter
            .or_else(|| env::var("EDGEE_LOG").ok())
            .or_else(|| env::var("RUST_LOG").ok())
            .unwrap_or_else(|| {
                if let Some(ref span) = config.span {
                    format!("edgee[{span}]={}", config.level)
                } else {
                    format!("edgee={}", config.level)
                }
            });

        builder.parse_lossy(directives)
    };

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}

pub fn init_cli() {
    use std::env;

    use tracing::Level;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    let fmt_layer = fmt::layer().with_target(false).without_time();

    let filter_layer = {
        let directives = env::var("EDGEE_LOG")
            .ok()
            .or_else(|| env::var("RUST_LOG").ok())
            .unwrap_or_default();

        EnvFilter::builder()
            .with_default_directive(Level::ERROR.into())
            .parse_lossy(directives)
    };

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
