use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ComponentsConfiguration {
    #[serde(default = "default_data_collection_components")]
    pub data_collection: Vec<DataCollectionComponents>,
    #[serde(default = "default_consent_mapping_components")]
    pub consent_mapping: Vec<ConsentMappingComponents>,
    pub cache: Option<PathBuf>,
}

fn default_data_collection_components() -> Vec<DataCollectionComponents> {
    vec![]
}

fn default_consent_mapping_components() -> Vec<ConsentMappingComponents> {
    vec![]
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct DataCollectionComponents {
    pub name: String,
    pub component: String,
    pub credentials: HashMap<String, String>,
    #[serde(default = "default_data_collection_component_config")]
    pub config: DataCollectionComponentConfig,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct DataCollectionComponentConfig {
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

fn default_data_collection_component_config() -> DataCollectionComponentConfig {
    DataCollectionComponentConfig {
        anonymization: true,
        default_consent: "pending".to_string(),
        track_event_enabled: true,
        user_event_enabled: true,
        page_event_enabled: true,
    }
}

fn default_true() -> bool {
    true
}

fn default_pending() -> String {
    "pending".to_string()
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ConsentMappingComponents {
    pub name: String,
    pub component: String,
    #[serde(default = "default_consent_mapping_component_config")]
    pub config: ConsentMappingComponentConfig,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ConsentMappingComponentConfig {
    pub cookie_name: String,
}

fn default_consent_mapping_component_config() -> ConsentMappingComponentConfig {
    ConsentMappingComponentConfig {
        cookie_name: "".to_string(),
    }
}
