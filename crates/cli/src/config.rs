use std::path::Path;

use edgee_server::config::StaticConfiguration;

fn read_config(path: Option<&Path>) -> Result<StaticConfiguration, String> {
    let toml_path = Path::new("edgee.toml");
    let yaml_path = Path::new("edgee.yaml");

    if let Some(path) = path {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .expect("provided configuration file does not have a format extension or is invalid");

        let config_data =
            std::fs::read_to_string(path).expect("should read provided configuration file");

        match extension {
            "toml" => {
                return toml::from_str(&config_data)
                    .map_err(|e| format!("should parse config file: {e}"))
            }
            "yml" | "yaml" => {
                return serde_yml::from_str(&config_data)
                    .map_err(|e| format!("should parse config file: {e}"));
            }
            _ => return Err("provided configuration file has an unknown extension".to_string()),
        }
    }

    match (toml_path.exists(), yaml_path.exists()) {
        (true, true) => {
            Err("both edgee.toml and edgee.yaml exist but only one is expected.".into())
        }
        (false, false) => {
            Err("no configuration file found, either edgee.toml or edgee.yaml is required.".into())
        }
        (true, false) => {
            let config_file = std::fs::read_to_string(toml_path).expect("should read edgee.toml");
            toml::from_str(&config_file).map_err(|e| format!("should parse config file: {}", e))
        }
        (false, true) => {
            let config_file = std::fs::read_to_string(yaml_path).expect("should read edgee.yaml");
            serde_yml::from_str(&config_file)
                .map_err(|e| format!("should parse config file: {}", e))
        }
    }
}

pub fn init(config_path: Option<&Path>, trace_component: Option<&str>) {
    let mut config = read_config(config_path).expect("should read config file");
    config.validate().unwrap();

    if let Some(component) = trace_component {
        config.log.trace_component = Some(component.to_string());
    }

    edgee_server::config::set(config);
}
