use std::collections::HashSet;
use std::sync::OnceLock;

use edgee_components_runtime::config::ComponentsConfiguration;
use edgee_dc_sdk::Autocapture;
use serde::Deserialize;
use tracing::level_filters::LevelFilter;
use tracing::warn;

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
        proxy_only: false,
        enforce_no_store_policy: false,
        inject_sdk: false,
        inject_sdk_position: "append".to_string(),
        autocapture: Autocapture::default(),
    }
}

impl StaticConfiguration {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let validators: Vec<Box<dyn Fn() -> Result<(), String>>> =
            vec![Box::new(|| self.validate_no_duplicate_domains())];

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
            let mut seen_redirection = HashSet::new();
            let mut redirection_duplicates = HashSet::new();
            for redirection in &route.redirections {
                if !seen_redirection.insert(&redirection.source) {
                    redirection_duplicates.insert(&redirection.source);
                }
            }

            if !redirection_duplicates.is_empty() {
                warn!("duplicate redirections found: {:?}", duplicates)
            }
        }

        if !duplicates.is_empty() {
            Err(format!("duplicate domains found: {duplicates:?}"))
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
    pub trace_component: Option<String>,
}

impl Default for LogConfiguration {
    fn default() -> Self {
        Self {
            level: LevelFilter::INFO,
            span: None,
            trace_component: None,
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
    #[serde(default)]
    pub redirections: Vec<RedirectionsConfiguration>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct RedirectionsConfiguration {
    pub source: String,
    pub target: String,
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
    #[serde(default)]
    pub proxy_only: bool,
    #[serde(default)]
    pub enforce_no_store_policy: bool,
    #[serde(default)]
    pub inject_sdk: bool,
    #[serde(default)]
    pub inject_sdk_position: String,
    #[serde(default)]
    pub autocapture: Autocapture,
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

pub fn set(config: StaticConfiguration) {
    CONFIG
        .set(config)
        .expect("should initialize config only once")
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

        routing: vec![RoutingConfiguration {
            domain: "test.com".to_string(),
            rules: vec![RoutingRulesConfiguration::default()],
            backends: vec![BackendConfiguration::default()],
            redirections: vec![RedirectionsConfiguration::default()],
        }],
        compute: default_compute_config(),
        components: ComponentsConfiguration::default(),
    };
    config.validate().unwrap();
    CONFIG.get_or_init(|| config);
}
