use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use edgee_api_client::types as api_types;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Manifest {
    pub manifest_version: u8,
    pub component: Component,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Component {
    pub name: String,
    pub slug: Option<String>,
    pub version: String,
    #[serde(with = "Category")]
    pub category: api_types::ComponentCreateInputCategory,
    #[serde(with = "SubCategory")]
    pub subcategory: api_types::ComponentCreateInputSubcategory,
    pub description: Option<String>,
    pub icon_path: Option<String>,
    #[serde(default)]
    pub language: Option<String>,

    #[serde(default)]
    pub documentation: Option<url::Url>,
    #[serde(default)]
    pub repository: Option<url::Url>,

    pub wit_version: String,

    pub build: Build,

    #[serde(default)]
    pub settings: IndexMap<String, Setting>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(
    remote = "api_types::ComponentCreateInputCategory",
    rename_all = "kebab-case"
)]
pub enum Category {
    DataCollection,
    EdgeFunction,
    JsGateway,
    Security,
    ConsentManagement,
    Identity,
    Stitching,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(
    remote = "api_types::ComponentCreateInputSubcategory",
    rename_all = "kebab-case"
)]
pub enum SubCategory {
    Analytics,
    Warehouse,
    Attribution,
    ConversionApi,
    WasmFunction,
    ServerSideTagging,
    Microservice,
    KvStore,
    BotProtection,
    RateLimiting,
    AntiFraud,
    ConsentMapping,
    Cmp,
    NativeCookies,
    UniqueId,
    AbTesting,
    WebPerformance,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Setting {
    pub title: String,
    #[serde(rename = "type", with = "SettingType")]
    pub type_: api_types::ConfigurationFieldType,
    #[serde(default)]
    pub required: bool,
    pub description: Option<String>,
    pub options: Option<Vec<String>>,
    #[serde(default)]
    pub secret: Option<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(
    remote = "api_types::ConfigurationFieldType",
    rename_all = "kebab-case"
)]
pub enum SettingType {
    String,
    Bool,
    Number,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Build {
    pub command: String,
    pub output_path: PathBuf,
}

impl Manifest {
    pub const VERSION: u8 = 1;
    pub const FILENAME: &str = "edgee-component.toml";

    pub fn load(path: &Path) -> Result<Self> {
        use std::fs;

        let content = fs::read_to_string(path)
            .with_context(|| format!("Could not read manifest file at {}", path.display()))?;

        let manifest: Self = toml::from_str(&content)
            .with_context(|| format!("Could not decode the manifest file at {}", path.display()))?;

        if manifest.manifest_version != Self::VERSION {
            anyhow::bail!(
                "Invalid manifest version {}, the supported one is {}",
                manifest.manifest_version,
                Self::VERSION
            );
        }

        if let Some(ref icon_path) = manifest.component.icon_path {
            let valid_extensions = ["png", "jpg", "jpeg"];
            let extension = Path::new(icon_path)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or_default()
                .to_lowercase();

            if !valid_extensions.contains(&extension.as_str()) {
                anyhow::bail!(
                    "Invalid icon path extension '{}', must be one of: {:?}",
                    extension,
                    valid_extensions
                );
            }
        }

        Ok(manifest)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        use std::fs;

        let content = toml::to_string(self)?;

        fs::write(path.join(Self::FILENAME), content)
            .with_context(|| format!("Could not write manifest file at {}", path.display()))?;
        Ok(())
    }
}

pub fn find_manifest_path() -> Option<PathBuf> {
    let mut cwd = std::env::current_dir().ok();

    while let Some(cur) = cwd {
        let path = cur.join(Manifest::FILENAME);
        if path.exists() {
            return Some(path);
        }

        cwd = cur.parent().map(ToOwned::to_owned);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn test_load_valid_manifest() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edgee-component.toml");
        let mut file = fs::File::create(&file_path).unwrap();

        writeln!(
            file,
            r#"
            manifest-version = 1
            [component]
            name = "test-component"
            version = "0.1.0"
            category = "data-collection"
            subcategory = "analytics"
            wit-version = "1.0.0"
            [component.build]
            command = "build"
            output_path = "file.wasm"
            "#
        )
        .unwrap();

        let manifest = Manifest::load(&file_path).unwrap();
        assert_eq!(manifest.manifest_version, 1);
        assert_eq!(manifest.component.name, "test-component");
    }

    #[test]
    #[serial]
    #[should_panic(expected = "Invalid manifest version")]
    fn test_load_invalid_manifest_version() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edgee-component.toml");
        let mut file = fs::File::create(&file_path).unwrap();

        writeln!(
            file,
            r#"
            manifest-version = 2
            [component]
            name = "test-component"
            version = "0.1.0"
            category = "data-collection"
            subcategory = "analytics"
            wit-version = "1.0.0"
            [component.build]
            command = "build"
            output_path = "file.wasm"
            "#
        )
        .unwrap();

        Manifest::load(&file_path).unwrap(); // should panic here
    }

    #[test]
    #[serial]
    #[should_panic(expected = "Could not decode the manifest file")]
    fn test_load_invalid_manifest_format() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edgee-component.toml");
        let mut file = fs::File::create(&file_path).unwrap();

        writeln!(
            file,
            r#"
            <some-xml>42</some-xml>
            "#
        )
        .unwrap();

        Manifest::load(&file_path).unwrap(); // should panic here
    }

    #[test]
    #[serial]
    #[should_panic(expected = "Invalid icon path extension")]
    fn test_load_invalid_icon_extension() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edgee-component.toml");
        let mut file = fs::File::create(&file_path).unwrap();

        writeln!(
            file,
            r#"
            manifest-version = 1
            [component]
            name = "test-component"
            version = "0.1.0"
            category = "data-collection"
            subcategory = "analytics"
            icon-path = "icon.bmp"
            wit-version = "1.0.0"
            [component.build]
            command = "build"
            output_path = "file.wasm"
            "#
        )
        .unwrap();

        Manifest::load(&file_path).unwrap(); // should panic here
    }

    #[test]
    #[serial]
    fn test_save_manifest() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edgee-component.toml");

        let manifest = Manifest {
            manifest_version: 1,
            component: Component {
                name: "test-component".to_string(),
                slug: None,
                version: "0.1.0".to_string(),
                category: api_types::ComponentCreateInputCategory::DataCollection,
                subcategory: api_types::ComponentCreateInputSubcategory::Analytics,
                description: Some("Test description".to_string()),
                icon_path: Some("image.png".to_string()),
                language: Some("Rust".to_string()),
                documentation: Some(url::Url::parse("https://github.com/test/test").unwrap()),
                repository: Some(url::Url::parse("https://github.com/test/test").unwrap()),
                wit_version: "1.0.0".to_string(),
                build: Build {
                    command: "build".to_string(),
                    output_path: PathBuf::from("file.wasm"),
                },
                settings: IndexMap::new(),
            },
        };

        manifest.save(dir.path()).unwrap();
        let loaded_manifest = Manifest::load(&file_path).unwrap();
        assert_eq!(loaded_manifest.manifest_version, 1);
        assert_eq!(loaded_manifest.component.name, "test-component");
    }

    #[test]
    #[serial]
    #[should_panic(expected = "Could not write manifest file")]
    fn test_save_manifest_fail() {
        let dir = tempdir().unwrap();
        let invalid_path = dir.path().join("invalid/edgee-component.toml");

        let manifest = Manifest {
            manifest_version: 1,
            component: Component {
                name: "test-component".to_string(),
                slug: None,
                version: "0.1.0".to_string(),
                category: api_types::ComponentCreateInputCategory::DataCollection,
                subcategory: api_types::ComponentCreateInputSubcategory::Analytics,
                description: Some("Test description".to_string()),
                icon_path: Some("image.png".to_string()),
                language: Some("Rust".to_string()),
                documentation: Some(url::Url::parse("https://github.com/test/test").unwrap()),
                repository: Some(url::Url::parse("https://github.com/test/test").unwrap()),
                wit_version: "1.0.0".to_string(),
                build: Build {
                    command: "build".to_string(),
                    output_path: PathBuf::from("file.wasm"),
                },
                settings: IndexMap::new(),
            },
        };

        manifest.save(&invalid_path).unwrap(); // should panic here
    }

    #[test]
    #[serial]
    fn test_find_manifest_path() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edgee-component.toml");
        fs::File::create(&file_path).unwrap();

        std::env::set_current_dir(dir.path()).unwrap();
        let found_path = find_manifest_path().unwrap();
        assert_eq!(
            found_path.canonicalize().unwrap(),
            file_path.canonicalize().unwrap()
        );
    }

    #[test]
    #[serial]
    fn test_find_manifest_path_parent() {
        let dir = tempdir().unwrap();
        let child_dir = dir.path().join("child");
        fs::create_dir(&child_dir).unwrap();
        let file_path = dir.path().join("edgee-component.toml");
        fs::File::create(&file_path).unwrap();

        // enter child dir and find the manifest in the parent dir
        std::env::set_current_dir(&child_dir).unwrap();
        let found_path = find_manifest_path().unwrap();
        assert_eq!(
            found_path.canonicalize().unwrap(),
            file_path.canonicalize().unwrap()
        );
    }

    #[test]
    #[serial]
    fn test_find_manifest_path_not_found() {
        let dir = tempdir().unwrap();
        // manifest file won't be found
        std::env::set_current_dir(dir.path()).unwrap();
        let result = find_manifest_path();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_component_deserialize() {
        let toml_str = r#"
            name = "test-component"
            version = "0.1.0"
            category = "data-collection"
            subcategory = "analytics"
            wit-version = "1.0.0"
            [build]
            command = "build"
            output_path = "file.wasm"
        "#;

        let component: Component = toml::from_str(toml_str).unwrap();
        assert_eq!(component.name, "test-component");
        assert_eq!(component.version, "0.1.0");
        assert_eq!(
            component.category,
            api_types::ComponentCreateInputCategory::DataCollection
        );
        assert_eq!(
            component.subcategory,
            api_types::ComponentCreateInputSubcategory::Analytics
        );
    }

    #[test]
    #[serial]
    fn test_setting_deserialize() {
        let toml_str = r#"
            title = "test-setting"
            type = "string"
            required = true
        "#;

        let setting: Setting = toml::from_str(toml_str).unwrap();
        assert_eq!(setting.title, "test-setting");
        assert_eq!(setting.type_, api_types::ConfigurationFieldType::String);
        assert!(setting.required);
    }
}
