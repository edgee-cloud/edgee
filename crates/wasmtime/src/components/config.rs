use std::{collections::HashMap, fs, path::PathBuf};

use reqwest::Url;
use serde::Deserialize;

use super::config_file::{ComponentsConfigurationFile, DataCollectionConfigurationFile};

#[derive(Debug, Default, Clone)]
pub struct ComponentsConfiguration {
    data_collection: Vec<DataCollectionConfiguration>,
    cache: Option<PathBuf>,
}

impl ComponentsConfiguration {
    pub fn get_collections(&self) -> Vec<DataCollectionConfiguration> {
        self.data_collection.clone()
    }

    pub fn get_gache(&self) -> Option<PathBuf> {
        self.cache.clone()
    }

    pub fn add_collection(
        &mut self,
        name: String,
        wasm_source: WasmSource,
        credentials: HashMap<String, String>,
        config: ComponentConfig,
    ) {
        let item = DataCollectionConfiguration {
            name,
            component: wasm_source,
            credentials,
            config,
        };
        self.data_collection.push(item);
    }
}

impl From<&ComponentsConfigurationFile> for ComponentsConfiguration {
    fn from(value: &ComponentsConfigurationFile) -> Self {
        Self {
            data_collection: value.get_collections().iter().map(|e| e.into()).collect(),
            cache: value.get_gache(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DataCollectionConfiguration {
    pub name: String,
    pub component: WasmSource,
    pub credentials: HashMap<String, String>,
    pub config: ComponentConfig,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ComponentConfig {
    #[serde(default = "default_true")]
    pub anonymization: bool,
    #[serde(default = "default_pending")]
    pub default_consent: String,
    #[serde(default = "default_true")]
    pub track_event_enabled: bool,
    #[serde(default = "default_true")]
    pub user_event_enabled: bool,
    #[serde(default = "default_true")]
    pub page_event_enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_pending() -> String {
    "pending".to_string()
}

#[derive(Debug, Clone)]
pub enum WasmSource {
    Path(String),
    Url(String),
    InMemory(Vec<u8>),
}

impl WasmSource {
    pub async fn resolve(&self) -> anyhow::Result<Vec<u8>> {
        match &self {
            WasmSource::Path(path) => {
                // let path = PathBuf::from(&path);
                fs::read(path).map_err(|e| {
                    anyhow::anyhow!("Error reading wasm binary at: {} error: {:?}", &path, e)
                })
            }
            WasmSource::Url(url_path) => {
                let url: Url = url_path
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Invalid url: {} error: {:?}", url_path, e))?;
                let response = reqwest::get(url).await.map_err(|e| {
                    anyhow::anyhow!("Error connecting to url: {} error: {:?}", url_path, e)
                })?;
                let data = response
                    .bytes()
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!("Error getting data from url: {} error: {:?}", url_path, e)
                    })?
                    .to_vec();
                Ok(data)
            }
            WasmSource::InMemory(vec) => Ok(vec.clone()),
        }
    }
}

impl From<&DataCollectionConfigurationFile> for DataCollectionConfiguration {
    fn from(value: &DataCollectionConfigurationFile) -> Self {
        Self {
            name: value.name.clone(),
            component: WasmSource::Path(value.component.clone()),
            credentials: value.credentials.clone(),
            config: value.config.clone(),
        }
    }
}

impl DataCollectionConfiguration {
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub async fn get_wasm_binary(&self) -> anyhow::Result<Vec<u8>> {
        self.component.resolve().await
    }

    pub fn get_credentials(&self) -> HashMap<String, String> {
        self.credentials.clone()
    }
}
