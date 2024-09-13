use crate::config::config;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, registry, EnvFilter};

const ACCEPTED_LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "fatal"];

pub fn init() {
    let level = &config::get().log.level;

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
