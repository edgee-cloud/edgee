use std::collections::HashMap;

use serde::Deserialize;
use tokio::sync::OnceCell;

static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

#[derive(Deserialize, Debug, Clone)]
pub struct StaticConfiguration {
    pub log: Option<LogConfiguration>,
    pub http: HttpConfiguration,
    pub https: HttpsConfiguration,
    pub monitor: Option<MonitorConfiguration>,
    #[serde(default)]
    pub routing: Vec<RoutingConfiguration>,
    #[serde(skip)]
    pub security: SecurityConfiguration,
    #[serde(default)]
    pub destinations: DestinationConfiguration,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogConfiguration {
    pub level: String,
}

#[derive(Deserialize, Debug, Clone)]
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
    pub max_compressed_body_size: Option<u64>,
    pub max_decompressed_body_size: Option<u64>,
}

impl Default for RoutingRulesConfiguration {
    fn default() -> Self {
        Self {
            path: Default::default(),
            path_prefix: Some(String::from("/")),
            path_regexp: Default::default(),
            rewrite: Default::default(),
            backend: Default::default(),
            max_compressed_body_size: Default::default(),
            max_decompressed_body_size: Default::default(),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BackendConfiguration {
    pub name: String,
    #[serde(default)]
    pub default: bool,
    pub address: String,
    pub enable_ssl: bool,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct DestinationConfiguration {
    pub data_collection: Vec<DataCollectionConfiguration>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct DataCollectionConfiguration {
    pub name: String,
    pub component: String,
    pub credentials: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SecurityConfiguration {
    pub cookie_name: String,
    pub aes_key: String,
    pub aes_iv: String,
}

impl Default for SecurityConfiguration {
    fn default() -> Self {
        Self {
            cookie_name: String::from("edgee"),
            aes_key: String::from("_key.edgee.cloud"),
            aes_iv: String::from("__iv.edgee.cloud"),
        }
    }
}

// TODO: Read config from CLI arguments
// TODO: Support YAML
// TODO: Support dynamic configuration via Redis
// TODO: Validate configuration (e.g. no two routers should point for the same domain)
// TODO: Improve error messages for configuration errors
pub fn init() {
    let config_file = std::fs::read_to_string("edgee.toml").expect("should read edgee.toml");
    let mut config: StaticConfiguration =
        toml::from_str(&config_file).expect("should parse config file");

    config.security = SecurityConfiguration::default();

    if let Some(key) = std::env::var("EDGEE_SECURITY_AES_KEY").ok() {
        config.security.aes_key = key;
    }

    if let Some(iv) = std::env::var("EDGEE_SECURITY_AES_IV").ok() {
        config.security.aes_iv = iv;
    }

    CONFIG.set(config).expect("Should initialize config");
}

pub fn get() -> &'static StaticConfiguration {
    CONFIG
        .get()
        .expect("config module should have been initialized")
}
