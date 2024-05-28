use tracing_subscriber::{fmt::Subscriber, util::SubscriberInitExt, EnvFilter};

use crate::config;

const ACCEPTED_LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "fatal"];

pub fn init() {
    let level = &config::get().log.level;

    if !ACCEPTED_LEVELS.contains(&level.as_str()) {
        panic!("Unsupported log level: {level}");
    }

    let filter: EnvFilter = level.into();
    Subscriber::builder()
        .with_env_filter(filter)
        .finish()
        .try_init()
        .unwrap();
}
