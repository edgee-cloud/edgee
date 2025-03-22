use crate::data_collection::version::DataCollectionProtocolVersion;
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

#[derive(Deserialize, Debug, Clone, Default)]
pub struct DataCollectionComponents {
    #[serde(skip_deserializing)]
    pub project_component_id: String,
    #[serde(skip_deserializing)]
    pub slug: String,
    pub id: String, // could be a slug (edgee/amplitude) or an alias (amplitude)
    pub file: String,
    #[serde(default)]
    pub settings: DataCollectionComponentSettings,
    pub version: DataCollectionProtocolVersion,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct DataCollectionComponentSettings {
    pub edgee_anonymization: bool,
    pub edgee_default_consent: String,
    pub edgee_track_event_enabled: bool,
    pub edgee_user_event_enabled: bool,
    pub edgee_page_event_enabled: bool,
    #[serde(flatten)]
    pub additional_settings: HashMap<String, String>,
}

impl Default for DataCollectionComponentSettings {
    fn default() -> Self {
        DataCollectionComponentSettings {
            edgee_anonymization: true,
            edgee_default_consent: "pending".to_string(),
            edgee_track_event_enabled: true,
            edgee_user_event_enabled: true,
            edgee_page_event_enabled: true,
            additional_settings: HashMap::new(),
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
