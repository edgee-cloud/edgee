use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub http_port: u16,
    pub https_port: u16,
    pub force_https: bool,
    pub log: LogConfig,
    pub providers: ProviderConfig,
}

#[derive(Deserialize)]
pub struct LogConfig {
    pub level: String,
}

#[derive(Deserialize)]
pub struct ProviderConfig {
    pub file: ProviderFileConfig,
}

#[derive(Deserialize)]
pub struct ProviderFileConfig {
    pub filename: String,
}

// TODO: Remove unwrap
pub fn parse(config_file: String) -> Config {
    let config_file = std::fs::read_to_string(config_file).unwrap();
    let config: Config = toml::from_str(&config_file).unwrap();
    config
}
