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
/// - `EDGEE_LOG` env variable, formatted for [tracing_subscriber::EnvFilter]
/// - `RUST_LOG` "standard" env variable, also formatted for [tracing_subscriber::EnvFilter]
/// - Config file, specifying level and optionally span
///
/// In the case something goes wrong with parsing of these directives, logging is done
/// using the log level defined in config
pub fn init(log_format: LogFormat) {
    use std::env;

    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    use crate::config::config;

    let config = &config::get().log;

    let fmt_layer = fmt::layer();
    let fmt_layer = match log_format {
        LogFormat::Basic | LogFormat::Pretty => fmt_layer.boxed(),
        LogFormat::Json => fmt_layer.json().boxed(),
    };

    let filter_layer = {
        let builder = EnvFilter::builder().with_default_directive(config.level.into());

        // Get logging directives from EDGEE_LOG or standard RUST_LOG env variables
        let directives = env::var("EDGEE_LOG")
            .or_else(|_| env::var("RUST_LOG"))
            .unwrap_or_else(|_| {
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
