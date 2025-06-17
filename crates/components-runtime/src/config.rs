use crate::{
    data_collection::versions::DataCollectionWitVersion,
    edge_function::versions::EdgeFunctionWitVersion,
};
use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ComponentsConfiguration {
    #[serde(default)]
    pub data_collection: Vec<DataCollectionComponents>,
    // NOTE: add other version here
    #[serde(default)]
    pub edge_function: Vec<EdgeFunctionComponents>,

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
    pub wit_version: DataCollectionWitVersion,
    #[serde(default)]
    pub event_filtering_rules: Vec<ComponentEventFilteringRule>,
    #[serde(default)]
    pub data_manipulation_rules: Vec<ComponentDataManipulationRule>,
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
pub struct EdgeFunctionComponents {
    #[serde(skip_deserializing)]
    pub slug: String,
    pub id: String, // could be a slug (edgee/amplitude) or an alias (amplitude)
    pub file: String,
    #[serde(default)]
    pub wit_version: EdgeFunctionWitVersion,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ComponentEventFilteringRule {
    pub name: String,
    pub event_types: Vec<String>,
    pub conditions: Vec<ComponentEventFilteringRuleCondition>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ComponentEventFilteringRuleCondition {
    pub field: String,
    pub operator: String,
    pub value: String,
}

impl Default for ComponentEventFilteringRule {
    fn default() -> Self {
        ComponentEventFilteringRule {
            name: "".to_string(),
            event_types: vec![],
            conditions: vec![],
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ComponentDataManipulationRule {
    pub name: String,
    pub event_types: Vec<String>,
    pub manipulations: Vec<ComponentDataManipulationRuleManipulation>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ComponentDataManipulationRuleManipulation {
    pub from_property: String,
    pub to_property: String,
    pub manipulation_type: String,
}

impl Default for ComponentDataManipulationRule {
    fn default() -> Self {
        ComponentDataManipulationRule {
            name: "".to_string(),
            event_types: vec![],
            manipulations: vec![],
        }
    }
}
