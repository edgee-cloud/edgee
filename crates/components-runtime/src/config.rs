use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ComponentsConfiguration {
    #[serde(default)]
    pub data_collection: Vec<DataCollectionComponents>,
    #[serde(default)]
    pub consent_mapping: Vec<ConsentMappingComponents>,
    pub cache: Option<PathBuf>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct DataCollectionComponents {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub component: String,
    pub credentials: HashMap<String, String>,
    #[serde(default)]
    pub config: DataCollectionComponentConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DataCollectionComponentConfig {
    #[serde(default = "default_true")]
    pub anonymization: bool,
    #[serde(default = "default_consent")]
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

fn default_consent() -> String {
    "pending".to_string()
}

impl Default for DataCollectionComponentConfig {
    fn default() -> Self {
        DataCollectionComponentConfig {
            anonymization: true,
            default_consent: default_consent(),
            track_event_enabled: true,
            user_event_enabled: true,
            page_event_enabled: true,
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ConsentMappingComponents {
    pub name: String,
    pub component: String,
    #[serde(default)]
    pub config: ConsentMappingComponentConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConsentMappingComponentConfig {
    pub cookie_name: String,
}

impl Default for ConsentMappingComponentConfig {
    fn default() -> Self {
        ConsentMappingComponentConfig {
            cookie_name: "".to_string(),
        }
    }
}
