use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;

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
    pub fn is_component_enabled(&self, name: &str) -> &bool {
        // if destinations is not set, return true
        if self.components.is_none() {
            return &true;
        }

        // get destinations.get("all")
        let all = self
            .components
            .as_ref()
            .unwrap()
            .get("all")
            .unwrap_or(&true);

        // check if the components is enabled
        if self.components.as_ref().unwrap().contains_key(name) {
            return self.components.as_ref().unwrap().get(name).unwrap();
        }
        all
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
