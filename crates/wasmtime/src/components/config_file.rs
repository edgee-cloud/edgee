use std::{collections::HashMap, fs, path::PathBuf};

use serde::Deserialize;

use super::config::ComponentConfig;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ComponentsConfigurationFile {
    data_collection: Vec<DataCollectionConfigurationFile>,
    cache: Option<PathBuf>,
}

impl ComponentsConfigurationFile {
    pub fn get_collections(&self) -> Vec<DataCollectionConfigurationFile> {
        self.data_collection.clone()
    }

    pub fn get_gache(&self) -> Option<PathBuf> {
        self.cache.clone()
    }
}
#[derive(Deserialize, Debug, Default, Clone)]
pub struct DataCollectionConfigurationFile {
    pub name: String,
    pub component: String,
    pub credentials: HashMap<String, String>,
    #[serde(default = "default_component_config")]
    pub config: ComponentConfig,
}

fn default_component_config() -> ComponentConfig {
    ComponentConfig {
        anonymization: true,
        default_consent: "pending".to_string(),
        track_event_enabled: true,
        user_event_enabled: true,
        page_event_enabled: true,
    }
}

impl DataCollectionConfigurationFile {
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_wasm_binary(&self) -> anyhow::Result<Vec<u8>> {
        let path = PathBuf::from(&self.component);
        fs::read(path).map_err(|e| {
            anyhow::anyhow!(
                "Error reading wasm binary at: {} error: {:?}",
                &self.component,
                e
            )
        })
    }

    pub fn get_credentials(&self) -> HashMap<String, String> {
        self.credentials.clone()
    }
}
