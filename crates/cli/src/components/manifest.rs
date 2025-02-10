use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use edgee_api_client::types as api_types;

pub const MANIFEST_VERSION: u8 = 1;
pub const MANIFEST_FILENAME: &str = "edgee-component.toml";

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub manifest_version: u8,
    pub package: Package,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(with = "Category")]
    pub category: api_types::ComponentCreateInputCategory,
    #[serde(with = "SubCategory")]
    pub subcategory: api_types::ComponentCreateInputSubcategory,
    pub description: Option<String>,

    #[serde(default)]
    pub documentation: Option<url::Url>,
    #[serde(default)]
    pub repository: Option<url::Url>,

    pub wit_world_version: String,

    #[serde(default)]
    pub config_fields: HashMap<String, ConfigField>,

    pub build: Build,
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
}

#[derive(Debug, Deserialize)]
pub struct ConfigField {
    pub title: String,
    #[serde(rename = "type", with = "ConfigFieldType")]
    pub type_: api_types::ConfigurationFieldType,
    #[serde(default)]
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(
    remote = "api_types::ConfigurationFieldType",
    rename_all = "kebab-case"
)]
pub enum ConfigFieldType {
    String,
    Boolean,
    Number,
}

#[derive(Debug, Deserialize)]
pub struct Build {
    pub command: String,
    pub output_path: PathBuf,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        use std::fs;

        let content = fs::read_to_string(path)
            .with_context(|| format!("Could not read manifest file at {}", path.display()))?;

        let manifest: Self = toml::from_str(&content)
            .with_context(|| format!("Could not decode the manifest file at {}", path.display()))?;

        if manifest.manifest_version != MANIFEST_VERSION {
            anyhow::bail!(
                "Invalid manifest version ({} != {})",
                manifest.manifest_version,
                MANIFEST_VERSION
            );
        }

        Ok(manifest)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        use std::fs;

        let content = toml::to_string(self)?;

        fs::write(path.join(MANIFEST_FILENAME), content)
            .with_context(|| format!("Could not write manifest file at {}", path.display()))?;
        Ok(())
    }
}

pub fn find_manifest_path() -> Option<PathBuf> {
    let mut cwd = std::env::current_dir().ok();

    while let Some(cur) = cwd {
        let path = cur.join(MANIFEST_FILENAME);
        if path.exists() {
            return Some(path);
        }

        cwd = cur.parent().map(ToOwned::to_owned);
    }

    None
}
