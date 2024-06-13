use serde::Deserialize;
use tokio::sync::OnceCell;

static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

#[derive(Deserialize, Debug, Clone)]
pub struct StaticConfiguration {
    pub log: LogConfiguration,
    pub http: HttpConfiguration,
    pub https: HttpsConfiguration,
    pub monitor: Option<MonitorConfiguration>,
    pub routing: Vec<RoutingConfiguration>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogConfiguration {
    pub level: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct HttpConfiguration {
    pub address: String,
    #[serde(default)]
    pub force_https: bool,
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

#[derive(Deserialize, Debug, Clone, Default)]
pub struct RoutingConfiguration {
    pub domain: String,
    #[serde(default)]
    pub rules: Vec<RoutingRulesConfiguration>,
    pub backends: Vec<BackendConfiguration>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RoutingRulesConfiguration {
    pub path: Option<String>,
    pub path_prefix: Option<String>,
    pub path_regexp: Option<String>,
    pub rewrite: Option<String>,
    pub backend: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BackendConfiguration {
    pub name: String,
    #[serde(default)]
    pub default: bool,
    pub address: String,
    pub enable_ssl: bool,
    // certificate
    // override_host
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
    CONFIG
        .get()
        .expect("config module should have been initialized")
}
