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
    #[serde(default = "default_force_https")]
    pub force_https: bool,
}

fn default_force_https() -> bool {
    true
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
    pub default_backend: String,
    #[serde(default = "default_routes")]
    pub rules: Vec<RoutingRulesConfiguration>,
}

fn default_routes() -> Vec<RoutingRulesConfiguration> {
    vec![]
}

#[derive(Deserialize, Debug, Clone)]
pub struct RoutingRulesConfiguration {
    #[serde(rename = "match")]
    pub pattern: String,
    pub service: String,
}

// TODO: Read config from CLI arguments
// TODO: Support YAML
// TODO: Support dynamic configuration via Redis
// TODO: Validate configuration (e.g. no two routers should point for the same domain)
pub fn init() {
    let config_file = std::fs::read_to_string("edgee.toml").expect("Should read edgee.toml");
    let config: StaticConfiguration =
        toml::from_str(&config_file).expect("Should parse config file");
    CONFIG.set(config).expect("Should initialize config");
}

pub fn get() -> &'static StaticConfiguration {
    CONFIG.get().unwrap()
}
