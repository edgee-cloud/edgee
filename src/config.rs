use serde::Deserialize;
use tokio::sync::OnceCell;

static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

#[derive(Deserialize, Debug, Clone)]
pub struct StaticConfiguration {
    pub http: HttpConfiguration,
    pub https: HttpsConfiguration,
    pub monitor: Option<MonitorConfiguration>,
    pub log: LogConfiguration,
    pub routers: Vec<RouterConfiguration>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct HttpConfiguration {
    pub address: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct HttpsConfiguration {
    pub address: String,
    pub cert: String,
    pub key: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MonitorConfiguration {
    pub address: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogConfiguration {
    pub level: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RouterConfiguration {
    pub name: String,
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
    let config_file = std::fs::read_to_string("edgee.toml").expect("Should read edgee.toml");
    let config: StaticConfiguration =
        toml::from_str(&config_file).expect("Should parse config file");
    CONFIG.set(config).expect("Should initialize config");
}

pub fn get() -> &'static StaticConfiguration {
    CONFIG.get().unwrap()
}
