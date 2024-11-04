use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Payload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<DataCollection>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Context {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<Client>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign: Option<Campaign>,

    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub session: Option<Session>,
}

impl Context {
    pub fn fill_in(&mut self, other: &Context) {
        if let Some(page) = &mut self.page {
            page.fill_in(&other.page.as_ref().unwrap());
        }
        if let Some(user) = &mut self.user {
            user.fill_in(&other.user.as_ref().unwrap());
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct DataCollection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<HashMap<String, bool>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Context>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<Event>>,
}

impl DataCollection {
    pub fn populate_event_contexts(&mut self) {
        let components = self.components.clone();

        // if events are set, we use the data collection context to fill in the missing fields
        if let Some(events) = &mut self.events {
            for event in events.iter_mut() {
                event.uuid = uuid::Uuid::new_v4().to_string();
                event.timestamp = Utc::now();

                // fill in the missing context fields
                if let Some(context) = &mut event.context {
                    context.fill_in(&self.context.clone().unwrap());
                } else {
                    event.context = self.context.clone();
                }

                if let Some(data) = &mut event.data {
                    if event.event_type == EventType::Page {
                        // data is a Page
                        if let EventData::Page(event_data) = data {
                            event_data
                                .fill_in(&self.context.clone().unwrap().page.clone().unwrap());
                        }
                    }

                    if event.event_type == EventType::User {
                        // data is an User
                        if let EventData::User(user_data) = data {
                            user_data.fill_in(&self.context.clone().unwrap().user.clone().unwrap());
                        }
                    }
                } else {
                    if event.event_type == EventType::Page {
                        event.data =
                            Some(EventData::Page(self.context.clone().unwrap().page.unwrap()));
                    }

                    if event.event_type == EventType::User {
                        event.data =
                            Some(EventData::User(self.context.clone().unwrap().user.unwrap()));
                    }
                }

                if event.components.is_none() {
                    event.components = components.clone();
                }
            }
        }
    }
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct Event {
    #[serde(skip_deserializing)]
    pub uuid: String,

    #[serde(skip_deserializing)]
    pub timestamp: DateTime<Utc>,

    #[serde(rename = "type")]
    pub event_type: EventType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<EventData>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Context>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<HashMap<String, bool>>,
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct EventHelper {
            #[serde(rename = "type")]
            event_type: EventType,
            data: Option<serde_json::Value>,
            context: Option<Context>,
            components: Option<HashMap<String, bool>>,
        }

        let helper = EventHelper::deserialize(deserializer)?;
        let data = match helper.event_type {
            EventType::Page => helper
                .data
                .map(|d| serde_json::from_value(d).map(EventData::Page))
                .transpose()
                .unwrap_or_default(),
            EventType::User => helper
                .data
                .map(|d| serde_json::from_value(d).map(EventData::User))
                .transpose()
                .unwrap_or_default(),
            EventType::Track => helper
                .data
                .map(|d| serde_json::from_value(d).map(EventData::Track))
                .transpose()
                .unwrap_or_default(),
        };

        Ok(Event {
            uuid: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type: helper.event_type,
            data,
            context: helper.context,
            components: helper.components,
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

        // check if the destination is enabled
        if self.components.as_ref().unwrap().contains_key(name) {
            return self.components.as_ref().unwrap().get(name).unwrap();
        }
        all
    }

    pub fn is_all_components_disabled(&self) -> bool {
        if self.components.is_none() {
            return false;
        }

        // iterate over all components and check if there is at least one enabled
        for (_, enabled) in self.components.as_ref().unwrap() {
            if *enabled {
                return false;
            }
        }

        true
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
pub enum EventData {
    Page(Page),
    User(User),
    Track(Track),
}

impl Default for EventData {
    fn default() -> Self {
        EventData::Page(Page::default())
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Page {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>, // Properties field is free-form
}

impl Page {
    fn fill_in(&mut self, other: &Page) {
        if self.name.is_none() {
            self.name = other.name.clone();
        }
        if self.category.is_none() {
            self.category = other.category.clone();
        }
        if self.keywords.is_none() {
            self.keywords = other.keywords.clone();
        }
        if self.title.is_none() {
            self.title = other.title.clone();
        }
        if self.url.is_none() {
            self.url = other.url.clone();
        }
        if self.path.is_none() {
            self.path = other.path.clone();
        }
        if self.search.is_none() {
            self.search = other.search.clone();
        }
        if self.referrer.is_none() {
            self.referrer = other.referrer.clone();
        }
        if self.properties.is_none() {
            self.properties = other.properties.clone();
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub anonymous_id: Option<String>,

    #[serde(skip_deserializing, default)]
    pub edgee_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>, // Properties field is free-form
}

impl User {
    fn fill_in(&mut self, other: &User) {
        if self.user_id.is_none() {
            self.user_id = other.user_id.clone();
        }
        if self.anonymous_id.is_none() {
            self.anonymous_id = other.anonymous_id.clone();
        }
        self.edgee_id = other.edgee_id.clone();
        if self.properties.is_none() {
            self.properties = other.properties.clone();
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Track {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>, // Properties field is free-form
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Campaign {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub creative_format: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketing_tactic: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Client {
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub ip: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub locale: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub accept_language: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub user_agent: Option<String>,

    // Low Entropy Client Hint Data - from sec-ch-ua header
    // The brand and version information for each brand associated with the browser, in a comma-separated list. ex: "Chromium;130|Google Chrome;130|Not?A_Brand;99"
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub user_agent_version_list: Option<String>,

    // Low Entropy Client Hint Data - from Sec-Ch-Ua-Mobile header
    // Indicates whether the browser is on a mobile device. ex: 0
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub user_agent_mobile: Option<String>,

    // Low Entropy Client Hint Data - from Sec-Ch-Ua-Platform header
    // The platform or operating system on which the user agent is running. Ex: macOS
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub os_name: Option<String>,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Arch header
    // User Agent Architecture. ex: arm
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent_architecture: Option<String>,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Bitness header
    // The "bitness" of the user-agent's underlying CPU architecture. This is the size in bits of an integer or memory address—typically 64 or 32 bits. ex: 64
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent_bitness: Option<String>,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Full-Version-List header
    // The brand and full version information for each brand associated with the browser, in a comma-separated list. ex: Chromium;112.0.5615.49|Google Chrome;112.0.5615.49|Not?A-Brand;99.0.0.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent_full_version_list: Option<String>,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Model header
    // The device model on which the browser is running. Will likely be empty for desktop browsers. ex: Nexus 6
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent_model: Option<String>,

    // High Entropy Client Hint Data - from Sec-Ch-Ua-Platform-Version header
    // The version of the operating system on which the user agent is running. Ex: 12.2.1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_density: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub continent: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
}

impl Client {
    pub fn anonymize_ip(&mut self) {
        if self.ip.is_none() {
            return;
        }

        use std::net::IpAddr;

        const KEEP_IPV4_BYTES: usize = 3;
        const KEEP_IPV6_BYTES: usize = 6;

        let ip: IpAddr = self.ip.clone().unwrap().parse().unwrap();
        let anonymized_ip = match ip {
            IpAddr::V4(ip) => {
                let mut data = ip.octets();
                data[KEEP_IPV4_BYTES..].fill(0);
                IpAddr::V4(data.into())
            }
            IpAddr::V6(ip) => {
                let mut data = ip.octets();
                data[KEEP_IPV6_BYTES..].fill(0);
                IpAddr::V6(data.into())
            }
        };

        self.ip = Some(anonymized_ip.to_string());
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Session {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_session_id: Option<String>,
    pub session_count: u32,
    pub session_start: bool,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}
