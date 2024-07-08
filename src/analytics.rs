use crate::cookie::EdgeeCookie;
use crate::html::Document;
use crate::real_ip;
use chrono::{DateTime, Utc};
use html_escape;
use http::uri::PathAndQuery;
use http::HeaderMap;
use json_comments::StripComments;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write;
use std::net::SocketAddr;

pub fn process_document(
    document: &Document,
    edgee_cookie: &EdgeeCookie,
    proto: &str,
    host: &str,
    requested_path: &PathAndQuery,
    request_headers: &HeaderMap,
    remote_addr: SocketAddr,
) -> Payload {
    let context = document.context.clone();
    let context = StripComments::new(context.as_bytes());
    let mut payload = serde_json::from_reader(context)
        .ok()
        .unwrap_or(Payload::default());

    payload.uuid = uuid::Uuid::new_v4().to_string();
    payload.timestamp = chrono::Utc::now();
    payload.event_type = Some(String::from("page"));

    let user_id = String::from(edgee_cookie.id);

    if payload.identify.is_none() {
        let mut identify = Identify::default();
        identify.edgee_id = user_id;
        payload.identify = Some(identify);
    }

    if payload.page.is_none() {
        payload.page = Some(Page::default());
    }

    let mut user_session = Session {
        session_id: edgee_cookie.ss.timestamp().to_string(),
        previous_session_id: None,
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
        user_session.previous_session_id = Some(edgee_cookie.ps.unwrap().timestamp().to_string());
    }

    payload.session = Some(user_session);

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

    let url = payload
        .page
        .as_ref()
        .and_then(|p| p.url.as_ref())
        .map(|u| html_escape::decode_html_entities(u).to_string())
        .unwrap_or_else(|| {
            if canonical_url.is_empty() {
                format!("{}://{}{}", proto, host, requested_path.path())
            } else {
                canonical_url.clone()
            }
        });

    payload.page.as_mut().unwrap().url = Some(url);

    let path = payload
        .page
        .as_ref()
        .and_then(|p| p.path.as_ref())
        .map(|p| html_escape::decode_html_entities(p).to_string())
        .unwrap_or_else(|| {
            if canonical_path.is_empty() {
                requested_path.path().to_string()
            } else {
                canonical_path.clone()
            }
        });
    payload.page.as_mut().unwrap().path = Some(path);

    let search = payload
        .page
        .as_ref()
        .and_then(|p| p.search.as_ref())
        .map(|s| {
            if s.starts_with("?") {
                s.clone()
            } else {
                "?".to_string() + s
            }
        })
        .map(|s| html_escape::decode_html_entities(&s).to_string())
        .or_else(|| requested_path.query().map(|qs| "?".to_string() + qs))
        .unwrap_or_else(|| String::new());
    if search == "?" || search == "" {
        payload.page.as_mut().unwrap().search = None;
    } else {
        payload.page.as_mut().unwrap().search = Some(search.clone());
    }

    let title = payload
        .page
        .as_ref()
        .and_then(|p| p.title.as_ref())
        .map(|t| html_escape::decode_html_entities(t).to_string())
        .unwrap_or(document.title.clone());
    payload.page.as_mut().unwrap().title = Some(title.clone());

    let keywords = payload
        .page
        .as_ref()
        .and_then(|k| k.keywords.as_ref())
        .map(|k| k.to_vec())
        .unwrap_or_else(|| {
            let keywords = document.keywords.clone();
            keywords
                .split(",")
                .map(|s| String::from(s.trim()))
                .collect()
        });
    payload.page.as_mut().unwrap().keywords = Some(keywords);

    if payload.page.as_ref().unwrap().referrer.is_none() {
        let referer = request_headers
            .get("referer")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();
        payload.page.as_mut().unwrap().referrer = if referer.is_empty() {
            None
        } else {
            Some(referer)
        };
    }

    if payload.client.is_none() {
        payload.client = Some(Client::default());
    }

    let user_agent = request_headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    payload.client.as_mut().unwrap().user_agent = Some(user_agent);

    let ip = real_ip::get(remote_addr, request_headers);
    payload.client.as_mut().unwrap().ip = Some(ip);

    let locale = request_headers
        .get("accept-language")
        .and_then(|h| h.to_str().ok())
        .map(|v| v.split(','))
        .map(|languages| languages.flat_map(|lang| lang.split(';')))
        .map(|mut lang| match lang.find(|lang| !lang.trim().is_empty()) {
            Some(lang) => lang.to_string(),
            None => String::from("en-US"),
        });
    payload.client.as_mut().unwrap().locale = locale;

    let x_forwarded_for = request_headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    payload.client.as_mut().unwrap().x_forwarded_for = Some(x_forwarded_for);

    if let Some(sec_ch_ua_arch) = request_headers
        .get("sec-ch-ua-arch")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().user_agent_architecture =
            Some(sec_ch_ua_arch.replace("\"", ""));
    }

    if let Some(sec_ch_ua_bitness) = request_headers
        .get("sec-ch-ua-bitness")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().user_agent_bitness =
            Some(sec_ch_ua_bitness.replace("\"", ""));
    }

    if let Some(sec_ch_ua) = request_headers
        .get("sec-ch-ua")
        .and_then(|h| h.to_str().ok())
    {
        payload
            .client
            .as_mut()
            .unwrap()
            .user_agent_full_version_list = Some(process_sec_ch_ua(sec_ch_ua));
    }

    if let Some(sec_ch_ua_mobile) = request_headers
        .get("sec-ch-ua-mobile")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().user_agent_mobile =
            Some(sec_ch_ua_mobile.replace("?", ""));
    }

    if let Some(sec_ch_ua_model) = request_headers
        .get("sec-ch-ua-model")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().user_agent_model = Some(sec_ch_ua_model.replace("\"", ""));
    }

    if let Some(sec_ch_ua_platform) = request_headers
        .get("sec-ch-ua-platform")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().os_name = Some(sec_ch_ua_platform.replace("\"", ""));
    }

    if let Some(sec_ch_ua_platform_version) = request_headers
        .get("sec-ch-ua-platform-version")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().os_version =
            Some(sec_ch_ua_platform_version.replace("\"", ""));
    }

    let map: HashMap<String, String> =
        url::form_urlencoded::parse(requested_path.query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();
    let utm_keys = [
        "utm_campaign",
        "utm_source",
        "utm_medium",
        "utm_term",
        "utm_content",
        "utm_creative_format",
        "utm_marketing_tactic",
    ];
    if utm_keys.iter().any(|key| map.contains_key(*key)) && payload.campaign.is_none() {
        payload.campaign = Some(Default::default());
    }
    if map.contains_key("utm_campaign") {
        payload.campaign.as_mut().unwrap().name =
            Some(map.get("utm_campaign").unwrap().to_string());
    }
    if map.contains_key("utm_source") {
        payload.campaign.as_mut().unwrap().source =
            Some(map.get("utm_source").unwrap().to_string());
    }
    if map.contains_key("utm_medium") {
        payload.campaign.as_mut().unwrap().medium =
            Some(map.get("utm_medium").unwrap().to_string());
    }
    if map.contains_key("utm_term") {
        payload.campaign.as_mut().unwrap().term = Some(map.get("utm_term").unwrap().to_string());
    }
    if map.contains_key("utm_content") {
        payload.campaign.as_mut().unwrap().content =
            Some(map.get("utm_content").unwrap().to_string());
    }
    if map.contains_key("utm_creative_format") {
        payload.campaign.as_mut().unwrap().creative_format =
            Some(map.get("utm_creative_format").unwrap().to_string());
    }
    if map.contains_key("utm_marketing_tactic") {
        payload.campaign.as_mut().unwrap().marketing_tactic =
            Some(map.get("utm_marketing_tactic").unwrap().to_string());
    }

    return payload;
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

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Payload {
    #[serde(skip_deserializing)]
    pub uuid: String,

    #[serde(skip_deserializing)]
    pub timestamp: DateTime<Utc>,

    #[serde(rename = "type")]
    pub event_type: Option<String>,

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

#[derive(Serialize, Deserialize, Debug, Default)]
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

#[derive(Serialize, Deserialize, Debug, Default)]
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

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Track {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>, // Properties field is free-form
}

#[derive(Serialize, Deserialize, Debug, Default)]
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

#[derive(Serialize, Deserialize, Debug, Default)]
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
    // todo geoip
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Session {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_session_id: Option<String>,
    pub session_count: u32,
    pub session_start: bool,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}
