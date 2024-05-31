use anyhow::Context;
use serde::Deserialize;
use tokio::sync::OnceCell;

static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

#[derive(Deserialize, Debug, Clone)]
pub struct StaticConfiguration {
    pub http: String,
    pub https: String,
    pub monitor: MonitorConfiguration,
    pub log: LogConfiguration,
    pub routers: Vec<RouterConfiguration>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MonitorConfiguration {
    http: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogConfiguration {
    pub level: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RouterConfiguration {
    pub name: String,
    pub entrypoints: Vec<String>,
    pub domain: String,
    pub routes: Vec<RouteConfiguration>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RouteConfiguration {
    #[serde(rename = "match")]
    pub pattern: String,
    pub service: String,
}

// TODO: Read config from CLI arguments
// TODO: Support YAML
// TODO: Support dynamic configuration via Redis
pub fn init() {
    let config_file = std::fs::read_to_string("edgee.toml").context("Failed to read edgee.toml");
    let config: StaticConfiguration =
        toml::from_str(&config_file).context("Failed to parse edgee.toml");
    CONFIG.set(config).context("Failed to initialize config");
}

pub fn get() -> &'static StaticConfiguration {
    CONFIG.get().unwrap()
}
