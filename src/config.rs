use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::Deserialize;
use tokio::sync::OnceCell;

static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

pub trait Validate {
    fn validate(&self) -> Result<(), Vec<String>>;
}

#[derive(Deserialize, Debug, Clone)]
pub struct StaticConfiguration {
    pub log: Option<LogConfiguration>,
    pub http: HttpConfiguration,
    pub https: HttpsConfiguration,

    // todo behind_proxy_cache
    // todo max_decompressed_body_size
    // todo max_compressed_body_size
    // todo cookie_name
    // todo data_collection_address
    // todo proxy_only

    pub monitor: Option<MonitorConfiguration>,
    #[serde(default)]
    pub routing: Vec<RoutingConfiguration>,
    #[serde(skip)]
    pub security: SecurityConfiguration,
    #[serde(default)]
    pub destinations: DestinationConfiguration,
}

impl Validate for StaticConfiguration {
    fn validate(&self) -> Result<(), Vec<String>> {
        let validators: Vec<Box<dyn Fn() -> Result<(), String>>> = vec![
            Box::new(|| self.validate_no_duplicate_domains()),
            // additional validation rules can be added here
        ];

        let errors: Vec<String> = validators
            .iter()
            .filter_map(|validate| validate().err())
            .collect();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl StaticConfiguration {
    fn validate_no_duplicate_domains(&self) -> Result<(), String> {
        let mut seen = HashSet::new();
        let mut duplicates = HashSet::new();

        for route in &self.routing {
            if !seen.insert(&route.domain) {
                duplicates.insert(&route.domain);
            }
        }

        if !duplicates.is_empty() {
            Err(format!("duplicate domains found: {:?}", duplicates))
        } else {
            Ok(())
        }
    }
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
    // todo, remove from here
    pub max_compressed_body_size: Option<u64>,
    // todo, remove from here
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

fn read_config() -> Result<StaticConfiguration, String> {
    let toml_exists = Path::new("edgee.toml").exists();
    let yaml_exists = Path::new("edgee.yaml").exists();

    match (toml_exists, yaml_exists) {
        (true, true) => {
            Err("both edgee.toml and edgee.yaml exist but only one is expected.".into())
        }
        (false, false) => {
            Err("no configuration file found, either edgee.toml or edgee.yaml is required.".into())
        }
        (true, false) => {
            let config_file =
                std::fs::read_to_string("edgee.toml").expect("should read edgee.toml");
            toml::from_str(&config_file).map_err(|_| "should parse config file".into())
        }
        (false, true) => {
            let config_file =
                std::fs::read_to_string("edgee.yaml").expect("should read edgee.yaml");
            serde_yml::from_str(&config_file).map_err(|_| "should parse config file".into())
        }
    }
}

// TODO: Read config from CLI arguments
// TODO: Add more configuration validations
// TODO: Improve error messages for configuration errors
pub fn init() {
    let mut config: StaticConfiguration = read_config().unwrap();
    config.validate().unwrap();

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

#[cfg(test)]
pub fn init_test_config() {
        let mut config = StaticConfiguration {
            log: Some(LogConfiguration { level: "debug".to_string() }),
            http: HttpConfiguration { address: "127.0.0.1:8080".to_string(), force_https: false },
            https: HttpsConfiguration { address: "127.0.0.1:8443".to_string(), cert: "cert.pem".to_string(), key: "key.pem".to_string() },
            monitor: Some(MonitorConfiguration { address: "127.0.0.1:9090".to_string() }),
            routing: vec![],
            security: SecurityConfiguration::default(),
            destinations: DestinationConfiguration::default(),
        };
        config.security = SecurityConfiguration::default();

        if CONFIG.get().is_none() {
            CONFIG.set(config).expect("Should initialize config");
        }
}
