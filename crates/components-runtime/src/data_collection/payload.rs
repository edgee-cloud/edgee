use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use tracing::error;

use crate::config::ComponentDataManipulationRule;
use crate::config::ComponentEventFilteringRuleCondition;
use crate::config::DataCollectionComponents;

pub type Dict = HashMap<String, serde_json::Value>;

#[derive(Serialize, Debug, Clone, Default)]
pub struct Event {
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub data: Data,
    pub context: Context,
    #[serde(skip_serializing)]
    pub components: Option<HashMap<String, bool>>,
    pub from: Option<String>,
    pub consent: Option<Consent>,
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct EventHelper {
            uuid: String,
            timestamp: DateTime<Utc>,
            #[serde(rename = "type")]
            event_type: EventType,
            #[serde(default)]
            data: serde_json::Value,
            #[serde(default)]
            context: Context,
            #[serde(default)]
            components: Option<HashMap<String, bool>>,
            #[serde(default)]
            from: Option<String>,
            #[serde(default)]
            consent: Option<Consent>,
        }

        let helper = EventHelper::deserialize(deserializer)?;
        let data = match helper.event_type {
            EventType::Page => Data::Page(serde_json::from_value(helper.data).unwrap()),
            EventType::User => Data::User(serde_json::from_value(helper.data).unwrap()),
            EventType::Track => {
                let mut track_data: Track = serde_json::from_value(helper.data).unwrap();

                // If products exist in properties, move them to the products field
                if let Some(serde_json::Value::Array(products_array)) =
                    track_data.properties.remove("products")
                {
                    track_data.products = products_array
                        .into_iter()
                        .filter_map(|p| {
                            if let serde_json::Value::Object(map) = p {
                                Some(map.into_iter().collect())
                            } else {
                                None
                            }
                        })
                        .collect();
                }

                Data::Track(track_data)
            }
        };

        Ok(Event {
            uuid: helper.uuid,
            timestamp: helper.timestamp,
            event_type: helper.event_type,
            data,
            context: helper.context,
            components: helper.components,
            from: helper.from,
            consent: helper.consent,
        })
    }
}

impl Event {
    pub fn is_component_enabled(&self, config: &DataCollectionComponents) -> &bool {
        // if destinations is not set, return true
        if self.components.is_none() {
            return &true;
        }

        let components = self.components.as_ref().unwrap();

        // get destinations.get("all")
        let all = components.get("all").unwrap_or(&true);

        // Check each possible key in order of priority
        for key in [&config.id, &config.project_component_id, &config.slug] {
            if let Some(enabled) = components.get(key.as_str()) {
                return enabled;
            }
        }

        all
    }
    pub fn should_filter_out(&self, condition: &ComponentEventFilteringRuleCondition) -> bool {
        let query = condition.field.as_str();
        let operator = condition.operator.as_str();
        let value = condition.value.as_str();

        let re = Regex::new(r"data\.(page|track|user)\.properties\.([a-zA-Z0-9_]+)").unwrap();
        if let Some(captures) = re.captures(query) {
            let data_type = captures.get(1).unwrap().as_str();
            let custom_field = captures.get(2).unwrap().as_str();

            match data_type {
                "page" => {
                    if let Data::Page(data) = &self.data {
                        if let Some(found_customer_property_value) =
                            data.properties.get(custom_field)
                        {
                            return evaluate_string_filter(
                                found_customer_property_value.to_string().as_str(),
                                operator,
                                value,
                            );
                        }
                    }
                }
                "track" => {
                    if let Data::Track(data) = &self.data {
                        if let Some(found_customer_property_value) =
                            data.properties.get(custom_field)
                        {
                            return evaluate_string_filter(
                                found_customer_property_value.to_string().as_str(),
                                operator,
                                value,
                            );
                        }
                    }
                }
                "user" => {
                    if let Data::User(data) = &self.data {
                        if let Some(found_customer_property_value) =
                            data.properties.get(custom_field)
                        {
                            return evaluate_string_filter(
                                found_customer_property_value.to_string().as_str(),
                                operator,
                                value,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        match query {
            "uuid" => evaluate_string_filter(&self.uuid, operator, value),
            "timestamp" => {
                let timestamp_f64 = self.timestamp.timestamp() as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&timestamp_f64, operator, &value_f64)
            }
            "timestamp-millis" => {
                let timestamp_f64 = self.timestamp.timestamp_millis() as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&timestamp_f64, operator, &value_f64)
            }
            "timestamp-micros" => {
                let timestamp_f64 = self.timestamp.timestamp_micros() as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&timestamp_f64, operator, &value_f64)
            }
            "event-type" => {
                let event_type_str = match self.event_type {
                    EventType::Page => "page",
                    EventType::User => "user",
                    EventType::Track => "track",
                };
                evaluate_string_filter(event_type_str, operator, value)
            }
            "consent" => {
                let consent_str = self.consent.as_ref().map_or("", |c| match c {
                    Consent::Granted => "granted",
                    Consent::Denied => "denied",
                    Consent::Pending => "pending",
                });
                evaluate_string_filter(consent_str, operator, value)
            }
            // Client fields
            "context.client.ip" => evaluate_string_filter(&self.context.client.ip, operator, value),
            "context.client.proxy-type" => {
                let proxy_type_str = self.context.client.proxy_type.as_deref().unwrap_or("");
                evaluate_string_filter(proxy_type_str, operator, value)
            }
            "context.client.proxy-desc" => {
                let proxy_desc_str = self.context.client.proxy_desc.as_deref().unwrap_or("");
                evaluate_string_filter(proxy_desc_str, operator, value)
            }
            "context.client.as-name" => {
                let as_name_str = self.context.client.as_name.as_deref().unwrap_or("");
                evaluate_string_filter(as_name_str, operator, value)
            }
            "context.client.as-number" => {
                let as_number_f64 = self.context.client.as_number.unwrap_or(0) as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&as_number_f64, operator, &value_f64)
            }
            "context.client.locale" => {
                evaluate_string_filter(&self.context.client.locale, operator, value)
            }
            "context.client.accept-language" => {
                evaluate_string_filter(&self.context.client.accept_language, operator, value)
            }
            "context.client.timezone" => {
                evaluate_string_filter(&self.context.client.timezone, operator, value)
            }
            "context.client.user-agent" => {
                evaluate_string_filter(&self.context.client.user_agent, operator, value)
            }
            "context.client.user-agent-version-list" => evaluate_string_filter(
                &self.context.client.user_agent_version_list,
                operator,
                value,
            ),
            "context.client.user-agent-mobile" => {
                evaluate_string_filter(&self.context.client.user_agent_mobile, operator, value)
            }
            "context.client.os-name" => {
                evaluate_string_filter(&self.context.client.os_name, operator, value)
            }
            "context.client.user-agent-architecture" => evaluate_string_filter(
                &self.context.client.user_agent_architecture,
                operator,
                value,
            ),
            "context.client.user-agent-bitness" => {
                evaluate_string_filter(&self.context.client.user_agent_bitness, operator, value)
            }
            "context.client.user-agent-full-version-list" => evaluate_string_filter(
                &self.context.client.user_agent_full_version_list,
                operator,
                value,
            ),
            "context.client.user-agent-model" => {
                evaluate_string_filter(&self.context.client.user_agent_model, operator, value)
            }
            "context.client.os-version" => {
                evaluate_string_filter(&self.context.client.os_version, operator, value)
            }
            "context.client.screen-width" => {
                let width_f64 = self.context.client.screen_width as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&width_f64, operator, &value_f64)
            }
            "context.client.screen-height" => {
                let height_f64 = self.context.client.screen_height as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&height_f64, operator, &value_f64)
            }
            "context.client.screen-density" => {
                let density_f64 = self.context.client.screen_density as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&density_f64, operator, &value_f64)
            }
            "context.client.continent" => {
                evaluate_string_filter(&self.context.client.continent, operator, value)
            }
            "context.client.country-code" => {
                evaluate_string_filter(&self.context.client.country_code, operator, value)
            }
            "context.client.country-name" => {
                evaluate_string_filter(&self.context.client.country_name, operator, value)
            }
            "context.client.region" => {
                evaluate_string_filter(&self.context.client.region, operator, value)
            }
            "context.client.city" => {
                evaluate_string_filter(&self.context.client.city, operator, value)
            }

            // Session fields
            "context.session.session-id" => {
                evaluate_string_filter(&self.context.session.session_id, operator, value)
            }
            "context.session.previous-session-id" => {
                evaluate_string_filter(&self.context.session.previous_session_id, operator, value)
            }
            "context.session.session-count" => {
                let count_f64 = self.context.session.session_count as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&count_f64, operator, &value_f64)
            }
            "context.session.session-start" => evaluate_boolean_filter(
                self.context.session.session_start,
                operator,
                value == "true",
            ),
            "context.session.first-seen" => {
                let timestamp_f64 = self.context.session.first_seen.timestamp() as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&timestamp_f64, operator, &value_f64)
            }
            "context.session.last-seen" => {
                let timestamp_f64 = self.context.session.last_seen.timestamp() as f64;
                let value_f64 = value.parse::<f64>().unwrap_or_default();
                evaluate_number_filter(&timestamp_f64, operator, &value_f64)
            }

            // Campaign fields
            "context.campaign.name" => {
                evaluate_string_filter(&self.context.campaign.name, operator, value)
            }
            "context.campaign.source" => {
                evaluate_string_filter(&self.context.campaign.source, operator, value)
            }
            "context.campaign.medium" => {
                evaluate_string_filter(&self.context.campaign.medium, operator, value)
            }
            "context.campaign.term" => {
                evaluate_string_filter(&self.context.campaign.term, operator, value)
            }
            "context.campaign.content" => {
                evaluate_string_filter(&self.context.campaign.content, operator, value)
            }
            "context.campaign.creative-format" => {
                evaluate_string_filter(&self.context.campaign.creative_format, operator, value)
            }
            "context.campaign.marketing-tactic" => {
                evaluate_string_filter(&self.context.campaign.marketing_tactic, operator, value)
            }

            // Page data fields
            "data.page.name" => {
                if let Data::Page(ref data) = self.data {
                    evaluate_string_filter(&data.name, operator, value)
                } else {
                    false
                }
            }
            "data.page.category" => {
                if let Data::Page(ref data) = self.data {
                    evaluate_string_filter(&data.category, operator, value)
                } else {
                    false
                }
            }
            "data.page.title" => {
                if let Data::Page(ref data) = self.data {
                    evaluate_string_filter(&data.title, operator, value)
                } else {
                    false
                }
            }
            "data.page.url" => {
                if let Data::Page(ref data) = self.data {
                    evaluate_string_filter(&data.url, operator, value)
                } else {
                    false
                }
            }
            "data.page.path" => {
                if let Data::Page(ref data) = self.data {
                    evaluate_string_filter(&data.path, operator, value)
                } else {
                    false
                }
            }
            "data.page.search" => {
                if let Data::Page(ref data) = self.data {
                    evaluate_string_filter(&data.search, operator, value)
                } else {
                    false
                }
            }
            "data.page.referrer" => {
                if let Data::Page(ref data) = self.data {
                    evaluate_string_filter(&data.referrer, operator, value)
                } else {
                    false
                }
            }

            // Track data fields
            "data.track.name" => {
                if let Data::Track(ref data) = self.data {
                    evaluate_string_filter(&data.name, operator, value)
                } else {
                    false
                }
            }

            // User data fields
            "data.user.user-id" => {
                if let Data::User(ref data) = self.data {
                    evaluate_string_filter(&data.user_id, operator, value)
                } else {
                    false
                }
            }
            "data.user.anonymous-id" => {
                if let Data::User(ref data) = self.data {
                    evaluate_string_filter(&data.anonymous_id, operator, value)
                } else {
                    false
                }
            }
            "data.user.edgee-id" => {
                if let Data::User(ref data) = self.data {
                    evaluate_string_filter(&data.edgee_id, operator, value)
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    pub fn apply_data_manipulation_rules(&mut self, rules: &[ComponentDataManipulationRule]) {
        rules.iter().for_each(|rule| {
            rule.event_types
                .iter()
                .for_each(|event_type| match event_type.as_str() {
                    "page" => {
                        if let Data::Page(ref mut data) = self.data {
                            rule.manipulations.iter().for_each(|manipulation| {
                                let value = data.properties.get(&manipulation.from_property);
                                if let Some(value) = value {
                                    data.properties
                                        .insert(manipulation.to_property.clone(), value.clone());
                                    data.properties.remove(&manipulation.from_property);
                                }
                            });
                        }
                    }
                    "track" => {
                        if let Data::Track(ref mut data) = self.data {
                            rule.manipulations.iter().for_each(|manipulation| {
                                if manipulation.manipulation_type == "replace-event-name" {
                                    if data.name == manipulation.from_property {
                                        data.name = manipulation.to_property.clone();
                                    }
                                } else {
                                    let value = data.properties.get(&manipulation.from_property);
                                    if let Some(value) = value {
                                        data.properties.insert(
                                            manipulation.to_property.clone(),
                                            value.clone(),
                                        );
                                        data.properties.remove(&manipulation.from_property);
                                    }
                                }
                            });
                        }
                    }
                    "user" => {
                        if let Data::User(ref mut data) = self.data {
                            rule.manipulations.iter().for_each(|manipulation| {
                                let value = data.properties.get(&manipulation.from_property);
                                if let Some(value) = value {
                                    data.properties
                                        .insert(manipulation.to_property.clone(), value.clone());
                                    data.properties.remove(&manipulation.from_property);
                                }
                            });
                        }
                    }
                    _ => {}
                });
        });
    }
}

pub fn evaluate_boolean_filter(field_value: bool, operator: &str, condition_value: bool) -> bool {
    match operator {
        "eq" => field_value == condition_value,
        "neq" => field_value != condition_value,
        _ => {
            error!("Invalid operator: {}", operator);
            false
        }
    }
}

pub fn evaluate_string_filter(field_value: &str, operator: &str, condition_value: &str) -> bool {
    match operator {
        "eq" => field_value == condition_value,
        "neq" => field_value != condition_value,
        "in" => condition_value.split(',').any(|v| v.trim() == field_value),
        "nin" => !condition_value.split(',').any(|v| v.trim() == field_value),
        "is_null" => field_value.is_empty(),
        "is_not_null" => !field_value.is_empty(),
        _ => {
            error!("Invalid operator: {}", operator);
            false
        }
    }
}

pub fn evaluate_number_filter(field_value: &f64, operator: &str, condition_value: &f64) -> bool {
    match operator {
        "eq" => field_value == condition_value,
        "neq" => field_value != condition_value,
        "gt" => field_value > condition_value,
        "lt" => field_value < condition_value,
        "gte" => field_value >= condition_value,
        "lte" => field_value <= condition_value,
        _ => {
            error!("Invalid operator: {}", operator);
            false
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum EventType {
    #[serde(rename = "page")]
    #[default]
    Page,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "track")]
    Track,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EventType::Page => write!(f, "page"),
            EventType::User => write!(f, "user"),
            EventType::Track => write!(f, "track"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Data {
    Page(Page),
    User(User),
    Track(Track),
}

impl Default for Data {
    fn default() -> Self {
        Data::Page(Page::default())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Consent {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "granted")]
    Granted,
    #[serde(rename = "denied")]
    Denied,
}

impl fmt::Display for Consent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Consent::Pending => write!(f, "pending"),
            Consent::Granted => write!(f, "granted"),
            Consent::Denied => write!(f, "denied"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Page {
    // skip serializing the default value
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    pub title: String,
    pub url: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub search: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub referrer: String,
    #[serde(default)]
    pub properties: Dict, // Properties field is free-form
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct User {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub anonymous_id: String,
    pub edgee_id: String,
    #[serde(default)]
    pub properties: Dict, // Properties field is free-form
    #[serde(skip_serializing)]
    pub native_cookie_ids: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Track {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub properties: Dict, // Properties field is free-form
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub products: Vec<Dict>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Context {
    #[serde(default)]
    pub page: Page,
    pub user: User,
    pub client: Client,
    #[serde(default)]
    pub campaign: Campaign,
    pub session: Session,
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Campaign {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub medium: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub term: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub creative_format: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub marketing_tactic: String,
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Client {
    pub ip: String,
    pub proxy_type: Option<String>,
    pub proxy_desc: Option<String>,
    pub as_name: Option<String>,
    pub as_number: Option<u32>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub locale: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub accept_language: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub timezone: String,
    pub user_agent: String,

    // Low Entropy Client Hint Data - from sec-ch-ua header
    // The brand and version information for each brand associated with the browser, in a comma-separated list. ex: "Chromium;130|Google Chrome;130|Not?A_Brand;99"
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_agent_version_list: String,

    // Low Entropy Client Hint Data - from Sec-Ch-Ua-Mobile header
    // Indicates whether the browser is on a mobile device. ex: 0
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_agent_mobile: String,

    // Low Entropy Client Hint Data - from Sec-Ch-Ua-Platform header
    // The platform or operating system on which the user agent is running. Ex: macOS
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub os_name: String,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Arch header
    // User Agent Architecture. ex: arm
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_agent_architecture: String,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Bitness header
    // The "bitness" of the user-agent's underlying CPU architecture. This is the size in bits of an integer or memory address—typically 64 or 32 bits. ex: 64
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_agent_bitness: String,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Full-Version-List header
    // The brand and full version information for each brand associated with the browser, in a comma-separated list. ex: Chromium;112.0.5615.49|Google Chrome;112.0.5615.49|Not?A-Brand;99.0.0.0
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_agent_full_version_list: String,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Model header
    // The device model on which the browser is running. Will likely be empty for desktop browsers. ex: Nexus 6
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_agent_model: String,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Platform-Version header
    // The version of the operating system on which the user agent is running. Ex: 12.2.1
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub os_version: String,

    #[serde(default)]
    pub screen_width: i32,

    #[serde(default)]
    pub screen_height: i32,

    #[serde(default)]
    pub screen_density: f32,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub continent: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub country_code: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub country_name: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub region: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub city: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Session {
    pub session_id: String,
    #[serde(default)]
    pub previous_session_id: String,
    pub session_count: u32,
    pub session_start: bool,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}
