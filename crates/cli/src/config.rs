use std::path::Path;

use edgee_proxy::config::StaticConfiguration;

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
                    .map_err(|e| format!("should parse valid toml file: {e}"))
            }
            "yml" | "yaml" => {
                return serde_yaml_ng::from_str(&config_data)
                    .map_err(|e| format!("should parse valid yaml file: {e}"));
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
            toml::from_str(&config_file).map_err(|e| format!("should parse valid toml file: {e}"))
        }
        (false, true) => {
            let config_file = std::fs::read_to_string(yaml_path).expect("should read edgee.yaml");
            serde_yaml_ng::from_str(&config_file)
                .map_err(|e| format!("should parse valid yaml file: {e}"))
        }
    }
}

pub fn init(config_path: Option<&Path>, trace_component: Option<&str>) {
    let mut config = read_config(config_path).expect("should read config file");
    config.validate().unwrap();

    if let Some(component) = trace_component {
        config.log.trace_component = Some(component.to_string());
    }

    edgee_proxy::config::set(config);
}
#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_temp_file(
        dir: &tempfile::TempDir,
        filename: &str,
        content: &str,
    ) -> std::path::PathBuf {
        let file_path = dir.path().join(filename);
        let mut file = File::create(&file_path).expect("should create temp file");
        file.write_all(content.as_bytes())
            .expect("should write to temp file");
        file_path
    }

    #[test]
    #[serial]
    fn test_read_config_with_toml_file() {
        let dir = tempdir().expect("should create temp dir");
        let toml_content = r#"
            [log]
            level = "info"
        "#;
        let toml_path = create_temp_file(&dir, "edgee.toml", toml_content);

        let config = read_config(Some(&toml_path)).expect("should read toml config");
        assert_eq!(config.log.level.to_string(), "info");
    }

    #[test]
    #[serial]
    #[should_panic(expected = "should parse valid toml file")]
    fn test_read_config_with_toml_file_invalid_content() {
        let dir = tempdir().expect("should create temp dir");
        let toml_content = r#"
             <some-xml>42</some-xml>
        "#;
        let toml_path = create_temp_file(&dir, "edgee.toml", toml_content);

        read_config(Some(&toml_path)).expect("should read toml config"); // should panic here
    }

    #[test]
    #[serial]
    fn test_read_config_with_toml_file_current_folder() {
        let dir = tempdir().expect("should create temp dir");
        let toml_content = r#"
            [log]
            level = "info"
        "#;
        create_temp_file(&dir, "edgee.toml", toml_content);

        std::env::set_current_dir(dir.path()).unwrap();
        let config = read_config(None).expect("should read toml config");
        assert_eq!(config.log.level.to_string(), "info");
    }

    #[test]
    #[serial]
    #[should_panic(expected = "should parse valid toml file")]
    fn test_read_config_with_toml_file_invalid_content_current_folder() {
        let dir = tempdir().expect("should create temp dir");
        let toml_content = r#"
             <some-xml>42</some-xml>
        "#;
        create_temp_file(&dir, "edgee.toml", toml_content);

        std::env::set_current_dir(dir.path()).unwrap();
        read_config(None).expect("should read toml config"); // should panic here
    }

    #[test]
    #[serial]
    fn test_read_config_with_yaml_file() {
        let dir = tempdir().expect("should create temp dir");
        let yaml_content = r#"
            log:
              level: "info"
        "#;
        let yaml_path = create_temp_file(&dir, "edgee.yaml", yaml_content);

        let config = read_config(Some(&yaml_path)).expect("should read yaml config");
        assert_eq!(config.log.level.to_string(), "info");
    }

    #[test]
    #[serial]
    #[should_panic(expected = "should parse valid yaml file")]
    fn test_read_config_with_yaml_file_invalid_content() {
        let dir = tempdir().expect("should create temp dir");
        let invalid_yaml_content = r#"
            <some-xml>42</some-xml>
        "#;
        let yaml_path = create_temp_file(&dir, "edgee.yaml", invalid_yaml_content);

        read_config(Some(&yaml_path)).expect("should read yaml config"); // should panic here
    }

    #[test]
    #[serial]
    fn test_read_config_with_yaml_file_current_folder() {
        let dir = tempdir().expect("should create temp dir");
        let yaml_content = r#"
            log:
              level: "info"
        "#;
        create_temp_file(&dir, "edgee.yaml", yaml_content);

        std::env::set_current_dir(dir.path()).unwrap();
        let config = read_config(None).expect("should read yaml config");
        assert_eq!(config.log.level.to_string(), "info");
    }

    #[test]
    #[serial]
    #[should_panic(expected = "should parse valid yaml file")]
    fn test_read_config_with_yaml_file_invalid_content_current_folder() {
        let dir = tempdir().expect("should create temp dir");
        let yaml_content = r#"
            <some-xml>42</some-xml>
        "#;
        create_temp_file(&dir, "edgee.yaml", yaml_content);

        std::env::set_current_dir(dir.path()).unwrap();
        read_config(None).expect("should read yaml config"); // should panic here
    }

    #[test]
    #[serial]
    #[should_panic(expected = "unknown extension")]
    fn test_read_config_with_invalid_extension() {
        let dir = tempdir().expect("should create temp dir");
        let invalid_content = r#"
            log:
              level: "info"
        "#;
        let path = create_temp_file(&dir, "edgee.txt", invalid_content);

        read_config(Some(&path)).expect("should read config"); // should panic here
    }

    #[test]
    #[serial]
    #[should_panic(expected = "no configuration file found")]
    fn test_read_config_with_no_files() {
        let dir = tempdir().expect("should create temp dir");
        std::env::set_current_dir(dir.path()).unwrap();
        read_config(None).expect("should read config"); // should panic here
    }

    #[test]
    #[serial]
    #[should_panic(expected = "only one is expected")]
    fn test_read_config_with_both_files() {
        let dir = tempdir().expect("should create temp dir");
        let toml_content = r#"
            [log]
            level = "info"
        "#;
        let yaml_content = r#"
            log:
              level: "info"
        "#;
        create_temp_file(&dir, "edgee.toml", toml_content);
        create_temp_file(&dir, "edgee.yaml", yaml_content);

        std::env::set_current_dir(dir.path()).unwrap();
        read_config(None).expect("should read config"); // should panic here
    }

    #[test]
    #[serial]
    fn test_init_with_trace_component() {
        let dir = tempdir().expect("should create temp dir");
        let toml_content = r#"
            [log]
            level = "info"
        "#;
        let toml_path = create_temp_file(&dir, "edgee.toml", toml_content);

        init(Some(&toml_path), Some("component"));
        let config = edgee_proxy::config::get();
        assert_eq!(config.log.level.to_string(), "info");
        assert_eq!(config.log.trace_component, Some("component".to_string()));
    }
}
