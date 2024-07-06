use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, registry, EnvFilter};

use crate::config;

const ACCEPTED_LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "fatal"];

pub fn init() {
    let warn_level = "warn".to_string();
    let level = match &config::get().log {
        Some(log) => &log.level,
        None => &warn_level,
    };

    if !ACCEPTED_LEVELS.contains(&level.as_str()) {
        panic!("Unsupported log level: {level}");
    }

    let filter: EnvFilter = level.into();
    if cfg!(debug_assertions) {
        registry().with(fmt::layer()).with(filter).init();
    } else {
        registry().with(fmt::layer().json()).with(filter).init();
    }
}
