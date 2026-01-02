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

    let config = &edgee_proxy::config::get().log;

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

    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    const DEFAULT_DIRECTIVE: &str = "info,wit_deps=warn";

    let fmt_layer = fmt::layer().with_target(false).without_time();

    let filter_layer = {
        let directives = env::var("EDGEE_LOG")
            .ok()
            .or_else(|| env::var("RUST_LOG").ok())
            .unwrap_or_else(|| DEFAULT_DIRECTIVE.to_string());

        EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .parse_lossy(directives)
    };

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}

const SENTRY_ENDPOINT: &str = "https://de4323866781f026a004320ac2478e26@o4507468622004224.ingest.de.sentry.io/4509230203600976";

pub fn init_sentry() -> sentry::ClientInitGuard {
    let endpoint =
        std::env::var("EDGEE_SENTRY_ENDPOINT").unwrap_or_else(|_| SENTRY_ENDPOINT.to_string());
    let opts = sentry::ClientOptions {
        release: sentry::release_name!(),
        auto_session_tracking: true,
        session_mode: sentry::SessionMode::Application,
        debug: std::env::var("EDGEE_SENTRY_DEBUG")
            .is_ok_and(|value| value == "1" || value == "true"),
        ..Default::default()
    };
    sentry::init((endpoint, opts))
}

pub fn report_error(err: anyhow::Error) {
    sentry_anyhow::capture_anyhow(&err);

    for (idx, err) in err.chain().enumerate() {
        if idx == 1 {
            tracing::error!("Caused by:");
        }
        let spacing = if idx > 0 { "  " } else { "" };
        tracing::error!("{spacing}{err}");
    }
}
