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
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Setting {
    pub title: String,
    #[serde(rename = "type", with = "SettingType")]
    pub type_: api_types::ConfigurationFieldType,
    #[serde(default)]
    pub required: bool,
    pub description: Option<String>,
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
