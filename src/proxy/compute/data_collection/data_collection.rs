use crate::config::config;
use crate::proxy::compute::data_collection::payload::{EventType, Payload};
use crate::proxy::compute::data_collection::{components, payload};
use crate::proxy::compute::html::Document;
use crate::tools::edgee_cookie::EdgeeCookie;
use base64::alphabet::STANDARD;
use base64::engine::general_purpose::PAD;
use base64::engine::GeneralPurpose;
use base64::Engine;
use bytes::Bytes;
use html_escape;
use http::uri::PathAndQuery;
use http::{header, HeaderMap};
use json_comments::StripComments;
use std::collections::HashMap;
use std::fmt::Write;
use std::io::Read;
use tracing::{info, warn};

pub async fn process_from_html(
    document: &Document,
    edgee_cookie: &EdgeeCookie,
    proto: &str,
    host: &str,
    path: &PathAndQuery,
    request_headers: &HeaderMap,
    client_ip: &String,
) -> Option<String> {
    let json_context = document.context.clone();
    let mut payload = Payload::default();
    if !json_context.is_empty() {
        // Clean the json_context from comments and spaces
        let stripped_context = StripComments::new(json_context.as_bytes());
        // populate the edgee payload from the json
        let payload_result = parse_payload(stripped_context);
        if payload_result.is_err() {
            warn!("Error parsing json payload: {:?}", payload_result.err());
        } else {
            payload = payload_result.unwrap();
        }
    }
    payload.uuid = uuid::Uuid::new_v4().to_string();
    payload.timestamp = chrono::Utc::now();
    payload.event_type = Some(EventType::Page);

    // add session info
    payload = add_session(payload, edgee_cookie);

    // add more info from the html or request
    payload = add_more_info_from_html_or_request(document, payload, proto, host, path);

    // add more info from the request
    payload = add_more_info_from_request(request_headers, payload, path, client_ip);

    // send the payload to the data collection components
    if let Err(err) = components::send_data_collection(&payload).await {
        warn!(?err, "failed to send data collection payload");
    }

    // send the payload to the edgee data-collection-api, but only if the api key and url are set
    if config::get().compute.data_collection_api_key.is_some()
        && config::get().compute.data_collection_api_url.is_some()
    {
        let api_key = config::get().compute.data_collection_api_key.as_ref()?;
        let api_url = config::get().compute.data_collection_api_url.as_ref()?;
        let payload_json = serde_json::to_string(&payload).unwrap();
        info!(target: "data_collection", payload = payload_json.as_str());
        let b64 = GeneralPurpose::new(&STANDARD, PAD).encode(format!("{}:", api_key));
        // now, we can send the payload to the edgee data-collection-api without waiting for the response
        tokio::spawn(async move {
            let _ = reqwest::Client::new()
                .post(api_url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Basic {}", b64))
                .body(payload_json)
                .send()
                .await;
        });
    }

    Option::from(payload.uuid)
}

pub async fn process_from_json(
    body: &Bytes,
    edgee_cookie: &EdgeeCookie,
    path: &PathAndQuery,
    request_headers: &HeaderMap,
    client_ip: &String,
) {
    // populate the edgee payload from the json
    let payload_result = parse_payload(body.as_ref());
    if payload_result.is_err() {
        warn!("Error parsing json payload: {:?}", payload_result.err());
        return;
    }
    let mut payload = payload_result.unwrap();

    payload.uuid = uuid::Uuid::new_v4().to_string();
    payload.timestamp = chrono::Utc::now();

    // add session info
    payload = add_session(payload, edgee_cookie);

    // add more info from the request
    payload = add_more_info_from_request(request_headers, payload, path, client_ip);

    // send the payload to the data collection components
    if let Err(err) = components::send_data_collection(&payload).await {
        warn!(?err, "failed to send data collection payload");
    }

    // send the payload to the edgee data-collection-api, but only if the api key and url are set
    if config::get().compute.data_collection_api_key.is_some()
        && config::get().compute.data_collection_api_url.is_some()
    {
        let api_key = config::get()
            .compute
            .data_collection_api_key
            .as_ref()
            .unwrap();
        let api_url = config::get()
            .compute
            .data_collection_api_url
            .as_ref()
            .unwrap();
        let payload_json = serde_json::to_string(&payload).unwrap();
        info!(target: "data_collection", payload = payload_json.as_str());
        let b64 = GeneralPurpose::new(&STANDARD, PAD).encode(format!("{}:", api_key));
        // now, we can send the payload to the edgee data-collection-api without waiting for the response
        tokio::spawn(async move {
            let _ = reqwest::Client::new()
                .post(api_url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Basic {}", b64))
                .body(payload_json)
                .send()
                .await;
        });
    }
}

/// Adds session information to the payload based on the provided `EdgeeCookie`.
///
/// # Arguments
/// - `payload`: The `Payload` object to be updated with session information.
/// - `edgee_cookie`: A reference to the `EdgeeCookie` containing session-related data.
///
/// # Returns
/// - `Payload`: The updated `Payload` object with session information.
fn add_session(mut payload: Payload, edgee_cookie: &EdgeeCookie) -> Payload {
    // edgee_id
    let user_id = edgee_cookie.id.to_string();
    if payload.identify.is_none() {
        payload.identify = Some(Default::default());
    }
    payload.identify.as_mut().unwrap().edgee_id = user_id;

    let mut user_session = payload::Session {
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
    payload
}

/// Adds more information from the HTML document or request to the payload.
///
/// # Arguments
/// - `document`: A reference to the `Document` containing the HTML content.
/// - `payload`: The `Payload` object to be updated with additional information.
/// - `proto`: A string slice representing the protocol (e.g., "http" or "https").
/// - `host`: A string slice representing the host.
/// - `incoming_path`: A reference to the `PathAndQuery` object representing the request path and query.
///
/// # Returns
/// - `Payload`: The updated `Payload` object with additional information from the HTML document or request.

fn add_more_info_from_html_or_request(
    document: &Document,
    mut payload: Payload,
    proto: &str,
    host: &str,
    incoming_path: &PathAndQuery,
) -> Payload {
    // if payload.page is None, we create it
    if payload.page.is_none() {
        payload.page = Some(Default::default());
    }

    // canonical url
    let mut canonical_url = document.canonical.clone();

    // if canonical is a relative url, we add the domain
    if !canonical_url.is_empty() && !canonical_url.starts_with("http") {
        canonical_url = format!("{}://{}{}", proto, host, canonical_url);
    }

    // canonical path
    let mut canonical_path = "".to_string();
    if !canonical_url.is_empty() {
        canonical_path = url::Url::parse(canonical_url.as_str())
            .map(|u| u.path().to_string())
            .unwrap_or("".to_string());
    }

    // url: we first try to get it from the payload, then from the canonical, and finally from the request
    let url = payload
        .page
        .as_ref()
        .and_then(|p| p.url.as_ref())
        .map(|u| u.to_string())
        .map(|u| html_escape::decode_html_entities(&u).to_string())
        .or_else(|| {
            if !canonical_url.is_empty() {
                Option::from(canonical_url.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| format!("{}://{}{}", proto, host, incoming_path.to_string()));
    payload.page.as_mut().unwrap().url = Some(url);

    // path: we first try to get it from the payload, then from the canonical, and finally from the request
    let path = payload
        .page
        .as_ref()
        .and_then(|p| p.path.as_ref())
        .map(|p| p.to_string())
        .map(|p| html_escape::decode_html_entities(&p).to_string())
        .or_else(|| {
            if !canonical_path.is_empty() {
                Option::from(canonical_path.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| incoming_path.path().to_string());
    payload.page.as_mut().unwrap().path = Some(path);

    // search: we first try to get it from the payload, and finally from the request
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
        .or_else(|| incoming_path.query().map(|qs| "?".to_string() + qs))
        .unwrap_or_else(|| "".to_string());
    if search == "?" || search == "" {
        // if search is = "?", we leave it blank
        payload.page.as_mut().unwrap().search = None;
    } else {
        payload.page.as_mut().unwrap().search = Some(search.clone());
    }

    // title: we first try to get it from the payload, and finally from the title html tag
    let title = payload
        .page
        .as_ref()
        .and_then(|p| p.title.as_ref())
        .map(|t| t.to_string())
        .map(|t| html_escape::decode_html_entities(&t).to_string())
        .or_else(|| {
            let title_from_html = document.title.clone();
            if !title_from_html.is_empty() {
                Option::from(title_from_html)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "".to_string());
    payload.page.as_mut().unwrap().title = Some(title.clone());

    // keywords: we first try to get it from the payload, and finally from the keywords meta tag
    let keywords = payload
        .page
        .as_ref()
        .and_then(|k| k.keywords.as_ref())
        .map(|k| k.to_vec())
        .or_else(|| {
            let keywords_string = document.keywords.clone();
            if !keywords_string.is_empty() {
                Option::from(
                    keywords_string
                        .split(",")
                        .map(|s| s.trim().to_string())
                        .collect::<Vec<String>>(),
                )
            } else {
                None
            }
        })
        .unwrap_or_else(|| Vec::new());
    payload.page.as_mut().unwrap().keywords = Some(keywords);

    payload
}

/// Adds more information to the payload from the request headers.
///
/// # Arguments
/// - `request_headers`: A reference to the `HeaderMap` containing the request headers.
/// - `payload`: The `Payload` object to be updated with additional information.
/// - `path`: A reference to the `PathAndQuery` object representing the request path and query.
/// - `client_ip`: A reference to a `String` containing the client's IP address.
///
/// # Returns
/// - `Payload`: The updated `Payload` object with additional information from the request headers.
fn add_more_info_from_request(
    request_headers: &HeaderMap,
    mut payload: Payload,
    path: &PathAndQuery,
    client_ip: &String,
) -> Payload {
    // first, prepare the payload
    if payload.client.is_none() {
        payload.client = Some(Default::default());
    }

    if payload.page.is_none() {
        payload.page = Some(Default::default());
    }

    // get referer from request if it is not already in the payload
    if payload.page.as_ref().unwrap().referrer.is_none() {
        let referer = request_headers
            .get(header::REFERER)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        payload.page.as_mut().unwrap().referrer = Some(referer.to_string());
    }
    // if the referer is empty, we remove it
    if payload
        .page
        .as_ref()
        .unwrap()
        .referrer
        .as_ref()
        .unwrap()
        .is_empty()
    {
        payload.page.as_mut().unwrap().referrer = None;
    }

    // user_agent
    let user_agent = request_headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    payload.client.as_mut().unwrap().user_agent = Some(user_agent.to_string());

    // client ip
    payload.client.as_mut().unwrap().ip = Some(client_ip.to_string());

    // locale
    let locale = get_preferred_language(request_headers);
    payload.client.as_mut().unwrap().locale = Some(locale);

    // x_forwarded_for
    let x_forwarded_for = request_headers
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    payload.client.as_mut().unwrap().x_forwarded_for = Some(x_forwarded_for.to_string());

    // sec-ch-ua-arch (user_agent_architecture)
    if let Some(sec_ch_ua_arch) = request_headers
        .get("Sec-Ch-Ua-Arch")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().user_agent_architecture =
            Some(sec_ch_ua_arch.replace("\"", ""));
    }

    // sec-ch-ua-bitness (user_agent_bitness)
    if let Some(sec_ch_ua_bitness) = request_headers
        .get("Sec-Ch-Ua-Bitness")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().user_agent_bitness =
            Some(sec_ch_ua_bitness.replace("\"", ""));
    }

    // sec-ch-ua (user_agent_full_version_list)
    if let Some(sec_ch_ua) = request_headers
        .get("Sec-Ch-Ua")
        .and_then(|h| h.to_str().ok())
    {
        payload
            .client
            .as_mut()
            .unwrap()
            .user_agent_full_version_list = Some(process_sec_ch_ua(sec_ch_ua));
    }

    // sec-ch-ua-mobile (user_agent_mobile)
    if let Some(sec_ch_ua_mobile) = request_headers
        .get("Sec-Ch-Ua-Mobile")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().user_agent_mobile =
            Some(sec_ch_ua_mobile.replace("?", ""));
    }

    // sec-ch-ua-model (user_agent_model)
    if let Some(sec_ch_ua_model) = request_headers
        .get("Sec-Ch-Ua-Model")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().user_agent_model = Some(sec_ch_ua_model.replace("\"", ""));
    }

    // sec-ch-ua-platform (os_name)
    if let Some(sec_ch_ua_platform) = request_headers
        .get("Sec-Ch-Ua-Platform")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().os_name = Some(sec_ch_ua_platform.replace("\"", ""));
    }

    // sec-ch-ua-platform-version (os_version)
    if let Some(sec_ch_ua_platform_version) = request_headers
        .get("Sec-Ch-Ua-Platform-Version")
        .and_then(|h| h.to_str().ok())
    {
        payload.client.as_mut().unwrap().os_version =
            Some(sec_ch_ua_platform_version.replace("\"", ""));
    }

    // campaign
    let map: HashMap<String, String> =
        url::form_urlencoded::parse(path.query().unwrap_or("").as_bytes())
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

    payload
}

/// Processes the `Sec-CH-UA` header to extract and format the user agent information.
///
/// # Arguments
/// - `header`: A string slice that holds the value of the `Sec-CH-UA` header.
///
/// # Returns
/// - `String`: A formatted string containing the user agent information, with each key-value pair separated by a semicolon,
///   and multiple pairs separated by a pipe character.
///
/// The function uses a regular expression to capture the key and version from the header.
/// It ensures that the version string has four parts by appending ".0" if necessary.
/// The formatted key-version pairs are then concatenated into a single string.
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

/// Extracts the preferred language from the `accept-language` header in the request.
///
/// # Arguments
/// - `request_headers`: A reference to the `HeaderMap` containing the request headers.
///
/// # Returns
/// - `String`: The preferred language extracted from the `accept-language` header, or "en-us" if no valid language is found.
fn get_preferred_language(request_headers: &HeaderMap) -> String {
    let accept_language_header = request_headers
        .get("accept-language")
        .and_then(|header| header.to_str().ok())
        .unwrap_or("");
    let languages = accept_language_header.split(",");
    for l in languages {
        let lang = l.split(";").next().unwrap_or("").trim();
        if !lang.is_empty() {
            return lang.to_lowercase();
        }
    }
    "en-us".to_string()
}

/// Parses a JSON payload from a reader and returns a `Result` containing the `Payload` or a `serde_json::Error`.
///
/// # Type Parameters
/// - `T`: A type that implements the `Read` trait, representing the input source of the JSON data.
///
/// # Arguments
/// - `clean_json`: An instance of type `T` that provides the JSON data to be parsed.
///
/// # Returns
/// - `Result<Payload, serde_json::Error>`: A `Result` that is `Ok` if the JSON data was successfully parsed into a `Payload`,
///   or `Err` if there was an error during parsing.
///
/// # Errors
/// This function will return a `serde_json::Error` if the JSON data cannot be parsed into a `Payload`.
fn parse_payload<T: Read>(clean_json: T) -> Result<Payload, serde_json::Error> {
    match serde_json::from_reader(clean_json) {
        Ok(payload) => Ok(payload),
        Err(e) => Err(e),
    }
}
