mod config {

    use serde::Deserialize;
    use tokio::sync::OnceCell;

    static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

    #[derive(Deserialize, Debug)]
    pub struct StaticConfiguration {
        pub http_port: u16,
        pub https_port: u16,
        pub log: LogConfiguration,
    }

    #[derive(Deserialize, Debug)]
    pub struct LogConfiguration {
        pub level: String,
    }

    pub fn init() {
        let config_file = std::fs::read_to_string("edgee.toml").unwrap();
        let config: StaticConfiguration = toml::from_str(&config_file).unwrap();
        CONFIG.set(config).unwrap();
    }

    pub fn get() -> &'static StaticConfiguration {
        CONFIG.get().unwrap()
    }
}

mod logger {
    use tracing_subscriber::{fmt::Subscriber, util::SubscriberInitExt, EnvFilter};

    use super::config;

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
}

fn main() {
    config::init();
    logger::init();

    println!("http_port: {}", config::get().http_port);
    println!("https_port: {}", config::get().https_port);
    println!("log severity: {}", config::get().log.level);
}
