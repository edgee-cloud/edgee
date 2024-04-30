use log::{LevelFilter};
use serde::{Deserialize, Serialize};
use simple_logger::SimpleLogger;
use std::path::PathBuf;

/// Represents the configuration of your Edgee proxy
///
/// The `Config` struct holds all the configuration data required for Edgee to function correctly.
///
/// # Fields
///
/// * `log_severity` - A string that represents the log severity level. This can be one of the following values: "DEFAULT", "DEBUG", "INFO", "NOTICE", "WARNING", "ERROR", "CRITICAL", "ALERT", "EMERGENCY", or "PANIC".
/// * `edgee_behind_proxy_cache` - A boolean that indicates whether Edgee is behind a proxy cache. This defaults to `false`.
/// * `force_https` - A boolean that indicates whether HTTPS should be forced on the frontend.
/// * `max_decompressed_body_size` - An integer that represents the maximum size in bytes of the decompressed body. This defaults to 6000000.
/// * `max_compressed_body_size` - An integer that represents the maximum size in bytes of the compressed body. This defaults to 3000000.
/// * `cookie_name` - A string that represents the name of the Edgee cookie. This defaults to "edgee".
/// * `backend` - A vector of `Backend` structs that represent the backends that can be used to route requests.
/// * `routing` - An optional vector of `Routing` structs that represent the routing rules. If this is `None`, no routing rules are defined.
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub log_severity: String,
    #[serde(default = "default_edgee_behind_proxy_cache")]
    pub edgee_behind_proxy_cache: bool,
    pub force_https: bool,
    #[serde(default = "default_max_decompressed_body_size")]
    pub max_decompressed_body_size: usize,
    #[serde(default = "default_max_compressed_body_size")]
    pub max_compressed_body_size: usize,
    #[serde(default = "default_cookie_name")]
    pub cookie_name: String,
    backend: Vec<Backend>,
    #[serde(default)]
    routing: Option<Vec<Routing>>,
}

fn default_edgee_behind_proxy_cache() -> bool {
    false
}
fn default_max_decompressed_body_size() -> usize {
    6000000
}
fn default_max_compressed_body_size() -> usize {
    3000000
}
fn default_cookie_name() -> String {
    "edgee".to_string()
}


/// Represents a backend in the configuration.
///
/// Each `Backend` struct represents a single backend that can be used to route requests.
///
/// # Fields
///
/// * `name` - A string that represents the name of the backend.
/// * `address` - A string that represents the address of the backend.
/// * `enable_ssl` - An optional boolean that, if present and set to `true`, indicates that SSL should be enabled for this backend.
/// * `check_certificate` - An optional string that, if present, represents the certificate that should be checked for this backend.
/// * `ca_certificate` - An optional string that, if present, represents the CA certificate for this backend.
/// * `sni_hostname` - An optional string that, if present, represents the SNI hostname for this backend.
/// * `default` - An optional boolean that, if present and set to `true`, indicates that this backend is the default backend.
/// * `override_host` - An optional string that, if present, represents the host that should be overridden for this backend.
#[derive(Serialize, Deserialize, Debug)]
struct Backend {
    name: String,
    address: String,

    #[serde(default)]
    enable_ssl: Option<bool>,
    #[serde(default)]
    check_certificate: Option<String>,
    #[serde(default)]
    ca_certificate: Option<String>,
    #[serde(default)]
    sni_hostname: Option<String>,
    #[serde(default)]
    default: Option<bool>,
    #[serde(default)]
    override_host: Option<String>,
}

/// Represents a routing rule in the configuration.
///
/// Each `Routing` struct represents a single routing rule that is used to determine the backend for a given request.
///
/// # Fields
///
/// * `path` - A string that represents the path for which this rule applies. This can be a regular expression if `regex` is set to `true`.
/// * `regex` - A boolean that indicates whether the `path` is a regular expression.
/// * `backend_name` - The name of the backend to which requests that match this rule should be routed.
/// * `rank` - An integer that represents the rank of this rule. Rules with lower ranks are evaluated before rules with higher ranks.
/// * `rewritepath_regex` - An optional string that, if present, represents a regular expression that is used to rewrite the path of requests that match this rule.
/// * `rewritepath_replace` - An optional string that, if present, is used as the replacement string when the `rewritepath_regex` is applied to the path of a request.
#[derive(Serialize, Deserialize, Debug)]
struct Routing {
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub regex: bool,
    pub backend_name: String,
    pub rank: i32,

    #[serde(default)]
    pub rewritepath_regex: Option<String>,
    #[serde(default)]
    pub rewritepath_replace: Option<String>,
}

/// Configures Edgee proxy based on the provided config file.
///
/// This function retrieves the configuration for the provided config file and parses it into a `Config` struct.
/// It sets the logger with the log severity level from the configuration.
/// Finally, it sets the backends for the configuration.
///
/// # Arguments
///
/// * `config_file` - A string slice that represents the path to the configuration file.
///
/// # Returns
///
/// * `Ok(Config)` - If the configuration is successfully retrieved and parsed, it returns the `Config` struct.
/// * `Err(&'static str)` - If the configuration cannot be retrieved or parsed, it returns a static string indicating the error.
///
/// # Errors
///
/// This function will return an error if the configuration cannot be retrieved for the provided config file or if the retrieved configuration cannot be parsed into a `Config` struct.
pub fn configure(config_file: &PathBuf) -> Result<Config, &'static str> {
    // Open config_file and get the string
    let config_str = std::fs::read_to_string(config_file);
    if config_str.is_err() {
        eprintln!("EARLY ERROR: Failed to read the configuration file '{:?}'", config_file);
        Err("Failed to read the configuration file")?;
    }


    // Parse the value into a struct
    let config_result = parse_config(config_str.unwrap().as_str());
    if config_result.is_err() {
        Err("Failed to parse the configuration file")?;
    }

    let config = config_result.unwrap();

    // set logger
    let log_level = map_log_level(&config.log_severity);
    SimpleLogger::new().with_level(log_level).init().unwrap();

    Ok(config)
}

/// Parses the given configuration string into a `Config` struct.
///
/// # Arguments
///
/// * `config_str` - A string slice that holds the configuration data in JSON format.
///
/// # Returns
///
/// * `Ok(Config)` - If the parsing is successful, it returns the `Config` struct.
/// * `Err(&'static str)` - If the parsing fails, it returns a static string indicating the error.
///
/// # Errors
///
/// This function will return an error if the provided string cannot be parsed into a `Config` struct.
fn parse_config(config_str: &str) -> Result<Config, &'static str> {
    match serde_json::from_str(config_str) {
        Ok(config) => Ok(config),
        Err(err) => {
            eprintln!("EARLY ERROR: Error parsing the configuration file: {:?}", err);
            Err("Failed to parse the configuration file")
        },
    }
}

/// Maps the log severity level from the Edgee to the standard log levels.
///
/// The Edgee API has more detailed log levels than the standard log levels. This function maps the Edgee log levels to the standard log levels.
///
/// # Arguments
///
/// * `log_severity` - A string slice that represents the log severity level from the Edgee API.
///
/// # Returns
///
/// * `LevelFilter` - The corresponding standard log level.
///
/// # Log Levels
///
/// * "DEFAULT" - Mapped to `LevelFilter::Info`. The log entry has no assigned severity level.
/// * "DEBUG" - Mapped to `LevelFilter::Debug`. Debug or trace information.
/// * "INFO" - Mapped to `LevelFilter::Info`. Routine information, such as ongoing status or performance.
/// * "NOTICE" - Mapped to `LevelFilter::Info`. Normal but significant events, such as start up, shut down, or a configuration change.
/// * "WARNING" - Mapped to `LevelFilter::Warn`. Warning events might cause problems.
/// * "ERROR" - Mapped to `LevelFilter::Error`. Error events are likely to cause problems.
/// * "CRITICAL" - Mapped to `LevelFilter::Error`. Critical events cause more severe problems or outages.
/// * "ALERT" - Mapped to `LevelFilter::Error`. A person must take an action immediately.
/// * "EMERGENCY" - Mapped to `LevelFilter::Error`. One or more systems are unusable.
/// * "PANIC" - Mapped to `LevelFilter::Error`. System is unusable.
/// * Any other value - Mapped to `LevelFilter::Info`.
fn map_log_level(log_severity: &str) -> LevelFilter {
    match log_severity {
        "DEFAULT" => LevelFilter::Info,
        "DEBUG" => LevelFilter::Debug,
        "INFO" => LevelFilter::Info,
        "NOTICE" => LevelFilter::Info,
        "WARNING" => LevelFilter::Warn,
        "ERROR" => LevelFilter::Error,
        "CRITICAL" => LevelFilter::Error,
        "ALERT" => LevelFilter::Error,
        "EMERGENCY" => LevelFilter::Error,
        "PANIC" => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}

// Determines the routing for a given request based on the configuration.
//
// This function first determines the default backend by iterating over the backends in the configuration.
// If no default backend is found, it returns an error.
// If no routing rules are defined in the configuration, it returns the default backend.
//
// If routing rules are defined, it orders them by rank and iterates over them.
// It selects the first rule that matches the request's path.
// If no rule matches, it returns the default backend.
//
// If a rule matches, and it has a rewrite path regex defined, it applies the rewrite path rule to the request's path.
//
// # Arguments
//
// * `request` - A mutable reference to the `Request` that needs to be routed.
// * `config` - A reference to the `Config` struct that contains the routing rules.
//
// # Returns
//
// * `Ok(String)` - If a backend is found for the request, it returns the backend's name as a string.
// * `Err(&'static str)` - If no default backend is found, it returns an error.
// pub fn routing(request: &mut Request, config: &Config) -> Result<String, &'static str> {
//     // Get the default backend
//     // find the config.backend that has default set to true
//     let mut default_backend = "";
//     for backend in &config.backend {
//         default_backend = &backend.name;
//         if backend.default.is_some() && backend.default.unwrap() {
//             default_backend = &backend.name;
//             break;
//         }
//     }
//
//     if default_backend.is_empty() {
//         return Err("No default backend found");
//     }
//
//     if config.routing.is_none() {
//         return Ok(default_backend.to_string());
//     }
//
//     let routing = config.routing.as_ref().unwrap();
//     let mut ordered_rules: HashMap<i32, &Routing> = HashMap::new();
//     let mut keys: Vec<i32> = Vec::new();
//     for rule in routing {
//         keys.push(rule.rank);
//         ordered_rules.insert(rule.rank, rule);
//     }
//     keys.sort();
//
//     // return the backend name if the request matches a routing rule
//     // iterate over the routing rules in the order they are defined
//     let url_path = request.get_url().path();
//     let mut selected_rule: &Routing = &Routing {
//         path: "".to_string(),
//         regex: false,
//         backend_name: "".to_string(),
//         rank: 0,
//         rewritepath_regex: None,
//         rewritepath_replace: None,
//     };
//     for k in keys {
//         let rule = &ordered_rules[&k];
//         if !rule.regex {
//             // if the path matches the rule, use the backend
//             if url_path == rule.path {
//                 selected_rule = rule;
//                 break;
//             }
//         } else {
//             // if the path matches the regex, use the backend
//             if regex::Regex::new(&rule.path).unwrap().is_match(url_path) {
//                 selected_rule = rule;
//                 break;
//             }
//         }
//     }
//
//     if selected_rule.backend_name.is_empty() {
//         return Ok(default_backend.to_string());
//     }
//
//     // apply the routing path rule
//     if selected_rule.rewritepath_regex.is_some() {
//         let re = regex::Regex::new(selected_rule.rewritepath_regex.as_ref().unwrap()).unwrap();
//         let new_path = re.replace_all(
//             url_path,
//             selected_rule.rewritepath_replace.as_ref().unwrap(),
//         );
//         request.set_path(&new_path.to_string());
//     }
//
//     return Ok(selected_rule.backend_name.to_string());
// }
