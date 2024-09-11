use crate::tools::edgee_cookie::EdgeeCookie;
use crate::tools::real_ip::Realip;
use crate::html::Document;
use chrono::{DateTime, Utc};
use html_escape;
use http::uri::PathAndQuery;
use http::HeaderMap;
use json_comments::StripComments;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Write;
use std::net::SocketAddr;

pub fn process_document(document: &Document, edgee_cookie: &EdgeeCookie, proto: &str, host: &str, requested_path: &PathAndQuery, request_headers: &HeaderMap, remote_addr: &SocketAddr) -> Payload {
    let context = document.context.clone();
    let context = StripComments::new(context.as_bytes());
    let mut payload = serde_json::from_reader(context)
        .ok()
        .unwrap_or(Payload::default());

    payload.uuid = uuid::Uuid::new_v4().to_string();
    payload.timestamp = chrono::Utc::now();
    payload.event_type = EventType::Page;

    let user_id = String::from(edgee_cookie.id);
    if payload.identify.user_id.is_empty() {
        payload.identify.user_id = user_id.clone();
    }

    let mut user_session = Session {
        session_id: edgee_cookie.ss.timestamp().to_string(),
        previous_session_id: String::from(""),
        session_count: edgee_cookie.sc,
        session_start: false,
        first_seen: edgee_cookie.fs,
        last_seen: edgee_cookie.ls,
    };

    // if ss (session start) is equal to ls (last seen), it is a new session
    if edgee_cookie.ss == edgee_cookie.ls {
        user_session.session_start = true;
    }

    // previous session id
    if edgee_cookie.ps.is_some() {
        user_session.previous_session_id = edgee_cookie.ps.unwrap().timestamp().to_string();
    }

    payload.session = user_session;

    let mut canonical_url = document.canonical.clone();
    if !canonical_url.is_empty() && !canonical_url.starts_with("http") {
        canonical_url = format!("{}://{}{}", proto, host, canonical_url);
    }

    let mut canonical_path = "".to_string();
    if !canonical_url.is_empty() {
        canonical_path = url::Url::parse(&canonical_url)
            .map(|u| u.path().to_string())
            .unwrap_or("".to_string());
    }

    payload.page.url = if payload.page.url.is_empty() {
        if canonical_url.is_empty() {
            format!("{}://{}{}", proto, host, requested_path.path())
        } else {
            canonical_url.clone()
        }
    } else {
        html_escape::decode_html_entities(&payload.page.url).to_string()
    };

    payload.page.path = if payload.page.path.is_empty() {
        if canonical_path.is_empty() {
            requested_path.path().to_string()
        } else {
            canonical_path.clone()
        }
    } else {
        html_escape::decode_html_entities(&payload.page.path).to_string()
    };

    payload.page.search = if payload.page.search.is_empty() {
        match requested_path.query() {
            Some(qs) => format!("?{}", qs),
            None => String::new(),
        }
    } else if payload.page.search.starts_with("?") {
        payload.page.search
    } else {
        format!("?{}", payload.page.search)
    };

    payload.page.title = if payload.page.title.is_empty() {
        document.title.clone()
    } else {
        html_escape::decode_html_entities(&payload.page.title).to_string()
    };

    payload.page.keywords = if payload.page.keywords.is_empty() {
        let keywords = document.keywords.clone();
        keywords
            .split(",")
            .map(|s| String::from(s.trim()))
            .collect()
    } else {
        payload.page.keywords
    };

    if payload.page.referrer.is_empty() {
        let referer = request_headers
            .get("referer")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();
        payload.page.referrer = referer;
    }

    payload.client.user_agent = request_headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();

    // client ip
    let realip = Realip::new();
    payload.client.ip = realip.get_from_request(&remote_addr, request_headers);

    payload.client.locale = get_preferred_language(request_headers);

    payload.client.x_forwarded_for = request_headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();

    if let Some(sec_ch_ua_arch) = request_headers
        .get("sec-ch-ua-arch")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.user_agent_architecture = sec_ch_ua_arch.replace("\"", "");
    }

    if let Some(sec_ch_ua_bitness) = request_headers
        .get("sec-ch-ua-bitness")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.user_agent_bitness = sec_ch_ua_bitness.replace("\"", "");
    }

    if let Some(sec_ch_ua) = request_headers
        .get("sec-ch-ua")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.user_agent_full_version_list = process_sec_ch_ua(sec_ch_ua);
    }

    payload.identify.edgee_id = user_id.clone();

    if let Some(sec_ch_ua_mobile) = request_headers
        .get("sec-ch-ua-mobile")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.user_agent_mobile = sec_ch_ua_mobile.replace("?", "");
    }

    if let Some(sec_ch_ua_model) = request_headers
        .get("sec-ch-ua-model")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.user_agent_model = sec_ch_ua_model.replace("\"", "");
    }

    if let Some(sec_ch_ua_platform) = request_headers
        .get("sec-ch-ua-platform")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.os_name = sec_ch_ua_platform.replace("\"", "");
        payload.identify.edgee_id = user_id.clone();
    }

    if let Some(sec_ch_ua_platform_version) = request_headers
        .get("sec-ch-ua-platform-version")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.os_version = sec_ch_ua_platform_version.replace("\"", "");
    }

    let map: HashMap<String, String> =
        url::form_urlencoded::parse(requested_path.query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();
    if map.contains_key("utm_campaign") {
        payload.campaign.name = map.get("utm_campaign").unwrap().to_string();
    }
    if map.contains_key("utm_source") {
        payload.campaign.source = map.get("utm_source").unwrap().to_string();
    }
    if map.contains_key("utm_medium") {
        payload.campaign.medium = map.get("utm_medium").unwrap().to_string();
    }
    if map.contains_key("utm_term") {
        payload.campaign.term = map.get("utm_term").unwrap().to_string();
    }
    if map.contains_key("utm_content") {
        payload.campaign.content = map.get("utm_content").unwrap().to_string();
    }
    if map.contains_key("utm_creative_format") {
        payload.campaign.creative_format = map.get("utm_creative_format").unwrap().to_string();
    }
    if map.contains_key("utm_marketing_tactic") {
        payload.campaign.marketing_tactic = map.get("utm_marketing_tactic").unwrap().to_string();
    }

    payload
}

fn get_preferred_language(request_headers: &HeaderMap) -> String {
    let accept_language_header_option = request_headers.get("accept-language");
    let accept_language_header = match accept_language_header_option {
        Some(header) => header.to_str().unwrap_or(""),
        None => "",
    };
    let languages = accept_language_header.split(",");
    for l in languages {
        let lang = l.split(";").next().unwrap_or("").trim();
        if !lang.is_empty() {
            return lang.to_lowercase();
        }
    }
    "en-us".to_string()
}

fn process_sec_ch_ua(header: &str) -> String {
    let mut output = String::new();
    let re = regex::Regex::new(r#""([^"]+)";v="([^"]+)""#).unwrap();

    let matches: Vec<_> = re.captures_iter(header).collect();

    for (i, cap) in matches.iter().enumerate() {
        let key = &cap[1];
        let version = &cap[2];

        // Split the version string into its parts and ensure it has four parts
        let mut parts: Vec<_> = version.split('.').collect();
        while parts.len() < 4 {
            parts.push("0");
        }
        let version_str = parts.join(".");

        // Add the key and version to the output string
        write!(output, "{};{}", key, version_str).unwrap(); // Using write! macro to append formatted string

        // Add a separator between key-value pairs, except for the last pair
        if i < matches.len() - 1 {
            output.push('|');
        }
    }

    output
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Payload {
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub page: Page,
    pub identify: Identify,
    pub track: Option<Track>,
    pub client: Client,
    pub campaign: Campaign,
    pub session: Session,
    pub destinations: HashMap<String, serde_json::Value>,
}

// impl Payload {
//     pub fn update(&mut self, headers: &HeaderMap) {}

//     fn extract_header(headers: &HeaderMap, key: impl AsHeaderName) -> String {
//         headers
//             .get(key)
//             .and_then(|h| h.to_str().ok())
//             .map(String::from)
//             .unwrap_or_default()
//     }
// }

#[derive(Clone, Debug, Deserialize)]
pub enum EventType {
    #[serde(rename = "page")]
    Page,
    #[serde(rename = "identify")]
    Identify,
    #[serde(rename = "track")]
    Track,
}

impl Default for EventType {
    fn default() -> Self {
        EventType::Page
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Page {
    pub name: String,
    pub category: String,
    pub keywords: Vec<String>,
    pub title: String,
    pub url: String,
    pub path: String,
    pub search: String,
    pub referrer: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Identify {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "anonymousId")]
    pub anonymous_id: String,
    #[serde(rename = "edgeeId")]
    pub edgee_id: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Track {
    pub name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Campaign {
    pub name: String,
    pub source: String,
    pub medium: String,
    pub term: String,
    pub content: String,
    #[serde(rename = "creativeFormat")]
    pub creative_format: String,
    #[serde(rename = "marketingTactic")]
    pub marketing_tactic: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Client {
    pub ip: String,
    #[serde(rename = "xForwardedFor")]
    pub x_forwarded_for: String,
    pub locale: String,
    pub timezone: String,
    #[serde(rename = "userAgent")]
    pub user_agent: String,
    #[serde(rename = "uaa")]
    pub user_agent_architecture: String,
    #[serde(rename = "uab")]
    pub user_agent_bitness: String,
    #[serde(rename = "uafvl")]
    pub user_agent_full_version_list: String,
    #[serde(rename = "uamb")]
    pub user_agent_mobile: String,
    #[serde(rename = "uam")]
    pub user_agent_model: String,
    #[serde(rename = "osName")]
    pub os_name: String,
    #[serde(rename = "osVersion")]
    pub os_version: String,
    #[serde(rename = "screenWidth")]
    pub screen_width: i32,
    #[serde(rename = "screenHeight")]
    pub screen_height: i32,
    #[serde(rename = "screenDensity")]
    pub screen_density: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Session {
    pub session_id: String,
    pub previous_session_id: String,
    pub session_count: u32,
    pub session_start: bool,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}
