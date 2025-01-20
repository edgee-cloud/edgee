use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ComponentsConfiguration {
    pub data_collection: Vec<DataCollectionConfiguration>,
    pub cache: Option<PathBuf>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct DataCollectionConfiguration {
    pub name: String,
    pub component: String,
    pub credentials: HashMap<String, String>,
    #[serde(default = "default_component_config")]
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

fn default_component_config() -> ComponentConfig {
    ComponentConfig {
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
