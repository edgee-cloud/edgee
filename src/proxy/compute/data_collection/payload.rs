use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Payload {
    #[serde(skip_deserializing)]
    pub uuid: String,

    #[serde(skip_deserializing)]
    pub timestamp: DateTime<Utc>,

    #[serde(rename = "type")]
    pub event_type: Option<EventType>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub identify: Option<Identify>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<Track>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<Client>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign: Option<Campaign>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub destinations: Option<HashMap<String, bool>>,

    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub session: Option<Session>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub enum EventType {
    #[serde(rename = "page")]
    #[default]
    Page,
    #[serde(rename = "identify")]
    Identify,
    #[serde(rename = "track")]
    Track,
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

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Identify {
    #[serde(rename = "userId", skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    #[serde(rename = "anonymousId", skip_serializing_if = "Option::is_none")]
    pub anonymous_id: Option<String>,

    #[serde(rename(serialize = "edgeeId"), skip_deserializing, default)]
    pub edgee_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>, // Properties field is free-form
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

    #[serde(rename = "creativeFormat", skip_serializing_if = "Option::is_none")]
    pub creative_format: Option<String>,

    #[serde(rename = "marketingTactic", skip_serializing_if = "Option::is_none")]
    pub marketing_tactic: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Client {
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub ip: Option<String>,

    #[serde(
        rename = "xForwardedFor",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    pub x_forwarded_for: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub locale: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    #[serde(rename = "userAgent", skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,

    // User Agent Architecture. ex: arm
    #[serde(
        rename = "uaa",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    pub user_agent_architecture: Option<String>,

    // The "bitness" of the user-agent's underlying CPU architecture. This is the size in bits of an integer or memory address—typically 64 or 32 bits. ex: 64
    #[serde(
        rename = "uab",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    pub user_agent_bitness: Option<String>,

    // The brand and full version information for each brand associated with the browser, in a comma-separated list. ex: Chromium;112.0.5615.49|Google%20Chrome;112.0.5615.49|Not%3AA-Brand;99.0.0.0
    #[serde(
        rename = "uafvl",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    pub user_agent_full_version_list: Option<String>,

    // Indicates whether the browser is on a mobile device. ex: 0
    #[serde(
        rename = "uamb",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    pub user_agent_mobile: Option<String>,

    // The device model on which the browser is running. Will likely be empty for desktop browsers. ex: Nexus 6
    #[serde(
        rename = "uam",
        skip_serializing_if = "Option::is_none",
        skip_deserializing
    )]
    pub user_agent_model: Option<String>,

    // The platform or operating system on which the user agent is running. Ex: macOS
    #[serde(rename = "osName", skip_serializing_if = "Option::is_none")]
    pub os_name: Option<String>,

    // The version of the operating system on which the user agent is running. Ex: 12.2.1
    #[serde(rename = "osVersion", skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,

    #[serde(rename = "screenWidth", skip_serializing_if = "Option::is_none")]
    pub screen_width: Option<i32>,

    #[serde(rename = "screenHeight", skip_serializing_if = "Option::is_none")]
    pub screen_height: Option<i32>,

    #[serde(rename = "screenDensity", skip_serializing_if = "Option::is_none")]
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
