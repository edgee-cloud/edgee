use tracing_subscriber::{fmt::Subscriber, util::SubscriberInitExt, EnvFilter};

const ACCEPTED_LOG_LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "fatal"];

pub(crate) fn init(log_level: &String) {
    if !ACCEPTED_LOG_LEVELS.contains(&log_level.as_str()) {
        panic!("Invalid log level: {}", log_level);
    }

    let filter: EnvFilter = log_level.into();
    let subscriber = Subscriber::builder().with_env_filter(filter).finish();
    subscriber.try_init().unwrap();
}
