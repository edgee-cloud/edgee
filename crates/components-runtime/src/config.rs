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
#[serde(default)]
pub struct DataCollectionComponentConfig {
    pub anonymization: bool,
    pub default_consent: String,
    pub track_event_enabled: bool,
    pub user_event_enabled: bool,
    pub page_event_enabled: bool,
}

impl Default for DataCollectionComponentConfig {
    fn default() -> Self {
        DataCollectionComponentConfig {
            anonymization: true,
            default_consent: "pending".to_string(),
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
