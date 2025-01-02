use std::collections::HashSet;
use std::sync::OnceLock;

use edgee_wasmtime::components::config::ComponentsConfiguration;
use serde::Deserialize;
use tracing::level_filters::LevelFilter;

static CONFIG: OnceLock<StaticConfiguration> = OnceLock::new();

#[derive(Deserialize, Debug, Clone, Default)]
pub struct StaticConfiguration {
    #[serde(default)]
    pub log: LogConfiguration,

    pub http: Option<HttpConfiguration>,
    pub https: Option<HttpsConfiguration>,

    #[serde(default = "default_compute_config")]
    pub compute: ComputeConfiguration,

    pub monitor: Option<MonitorConfiguration>,

    #[serde(default)]
    pub routing: Vec<RoutingConfiguration>,

    #[serde(default)]
    pub components: ComponentsConfiguration,
}

fn default_compute_config() -> ComputeConfiguration {
    ComputeConfiguration {
        cookie_name: default_cookie_name(),
        cookie_domain: None,
        aes_key: default_aes_key(),
        aes_iv: default_aes_iv(),
        behind_proxy_cache: false,
        max_decompressed_body_size: default_max_decompressed_body_size(),
        max_compressed_body_size: default_max_compressed_body_size(),
        proxy_only: false,
        enforce_no_store_policy: false,
        data_collection_api_key: None,
        data_collection_api_url: None,
    }
}

impl StaticConfiguration {
    pub fn validate(&self) -> Result<(), Vec<String>> {
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

#[serde_with::serde_as]
#[derive(Deserialize, Debug, Clone)]
pub struct LogConfiguration {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub level: LevelFilter,
    pub span: Option<String>,
    pub debug_component: Option<String>,
}

impl Default for LogConfiguration {
    fn default() -> Self {
        Self {
            level: LevelFilter::INFO,
            span: None,
            debug_component: None,
        }
    }
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
    // todo change by host
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

impl Default for RoutingRulesConfiguration {
    fn default() -> Self {
        Self {
            path: Default::default(),
            path_prefix: Some(String::from("/")),
            path_regexp: Default::default(),
            rewrite: Default::default(),
            backend: Default::default(),
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
    #[serde(default)]
    pub compress: bool,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct ComputeConfiguration {
    #[serde(default = "default_cookie_name")]
    pub cookie_name: String,
    pub cookie_domain: Option<String>,
    #[serde(default = "default_aes_key")]
    pub aes_key: String,
    #[serde(default = "default_aes_iv")]
    pub aes_iv: String,
    #[serde(default)]
    pub behind_proxy_cache: bool,
    #[serde(default = "default_max_decompressed_body_size")]
    pub max_decompressed_body_size: usize,
    #[serde(default = "default_max_compressed_body_size")]
    pub max_compressed_body_size: usize,
    #[serde(default)]
    pub proxy_only: bool,
    #[serde(default)]
    pub enforce_no_store_policy: bool,
    pub data_collection_api_key: Option<String>,
    pub data_collection_api_url: Option<String>,
}

fn default_cookie_name() -> String {
    "edgee".to_string()
}
fn default_aes_key() -> String {
    "_key.edgee.cloud".to_string()
}
fn default_aes_iv() -> String {
    "__iv.edgee.cloud".to_string()
}
fn default_max_decompressed_body_size() -> usize {
    6000000
}
fn default_max_compressed_body_size() -> usize {
    3000000
}

pub fn set(config: StaticConfiguration) {
    CONFIG.set(config).expect("should initialize config")
}

pub fn get() -> &'static StaticConfiguration {
    CONFIG
        .get()
        .expect("config module should have been initialized")
}

#[cfg(test)]
pub fn init_test_config() {
    let config = StaticConfiguration {
        log: Default::default(),
        http: Some(HttpConfiguration {
            address: "127.0.0.1:8080".to_string(),
            force_https: false,
        }),
        https: Some(HttpsConfiguration {
            address: "127.0.0.1:8443".to_string(),
            cert: "cert.pem".to_string(),
            key: "key.pem".to_string(),
        }),
        monitor: Some(MonitorConfiguration {
            address: "127.0.0.1:9090".to_string(),
        }),
        routing: vec![],
        compute: default_compute_config(),
        components: ComponentsConfiguration::default(),
    };

    CONFIG.get_or_init(|| config);
}
