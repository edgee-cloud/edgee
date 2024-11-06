use crate::config::config;
use crate::proxy::compute::data_collection::payload::{EventData, EventType, Payload};
use crate::proxy::compute::data_collection::{components, payload};
use crate::proxy::compute::html::Document;
use crate::proxy::context::incoming::RequestHandle;
use crate::tools::edgee_cookie;
use base64::alphabet::STANDARD;
use base64::engine::general_purpose::PAD;
use base64::engine::GeneralPurpose;
use base64::Engine;
use bytes::Bytes;
use html_escape;
use http::response::Parts;
use http::{header, HeaderMap};
use json_comments::StripComments;
use regex::Regex;
use std::collections::HashMap;
use std::fmt::Write;
use std::io::Read;
use tracing::{info, warn, Instrument};

#[tracing::instrument(name = "data_collection", skip(document, request, response))]
pub async fn process_from_html(
    document: &Document,
    request: &RequestHandle,
    response: &mut Parts,
) -> Option<String> {
    let json_data_layer = document.data_layer.clone();
    let mut payload = Payload::default();
    if !json_data_layer.is_empty() {
        // Clean the json_data_layer from comments and spaces
        let stripped_data_layer = StripComments::new(json_data_layer.as_bytes());
        // populate the edgee data_layer from the json
        let payload_result = parse_payload(stripped_data_layer);
        if payload_result.is_err() {
            warn!("Error parsing json payload: {:?}", payload_result.err());
        } else {
            payload = payload_result.unwrap();
        }
    }

    // prepare the payload for data collection
    payload = prepare_data_collection_payload(payload);

    // add session info
    payload = add_session(request, response, payload);

    // add more info from the html or request
    payload = add_more_info_from_html_or_request(request, document, payload);

    // add more info from the request
    payload = add_more_info_from_request(request, payload);

    // populate the events with the data collection context
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .populate_event_contexts();

    // check if payload.events is empty, if so, add a page event
    if payload.data_collection.as_ref().unwrap().events.is_none() {
        // add a page event
        payload.data_collection.as_mut().unwrap().events = Some(vec![payload::Event {
            uuid: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: EventType::Page,
            data: Some(EventData::Page(
                payload
                    .data_collection
                    .clone()
                    .unwrap()
                    .context
                    .clone()
                    .unwrap()
                    .page
                    .unwrap(),
            )),
            context: payload.data_collection.clone().unwrap().context.clone(),
            // fill in components with payload.data_collection.unwrap().components if exists
            components: None,
        }]);
    }

    let mut events = payload
        .data_collection
        .clone()
        .unwrap()
        .events
        .unwrap_or_default();

    // remove events with all components disabled
    for e in events.clone().iter() {
        if e.is_all_components_disabled() {
            events.retain(|evt| evt.uuid != e.uuid);
        }
    }

    if events.is_empty() {
        return Option::from("[]".to_string());
    }

    let events_json =
        serde_json::to_string(&events).expect("Could not encode data collection events into JSON");
    info!(events = events_json.as_str());

    // send the payload to the data collection components
    tokio::spawn(
        async move {
            if let Err(err) = components::send_data_collection(&events).await {
                warn!(?err, "failed to send data collection payload");
            }
        }
        .in_current_span(),
    );

    // send the payload to the edgee data-collection-api, but only if the api key and url are set
    if config::get().compute.data_collection_api_key.is_some()
        && config::get().compute.data_collection_api_url.is_some()
    {
        let api_key = config::get().compute.data_collection_api_key.as_ref()?;
        let api_url = config::get().compute.data_collection_api_url.as_ref()?;

        let b64 = GeneralPurpose::new(&STANDARD, PAD).encode(format!("{}:", api_key));
        let events_json = events_json.clone();
        // now, we can send the payload to the edgee data-collection-api without waiting for the response
        let debug = if request.is_debug_mode() {
            "true"
        } else {
            "false"
        };
        let host = request.get_host().to_string();
        tokio::spawn(async move {
            let _ = reqwest::Client::new()
                .post(api_url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Basic {}", b64))
                .header("X-Edgee-Debug", debug)
                .header("X-Edgee-From", "edge")
                .header("X-Edgee-Host", host)
                .body(events_json)
                .send()
                .await;
        });
    }

    Option::from(events_json)
}

#[tracing::instrument(name = "data_collection", skip(body, request, response))]
pub async fn process_from_json(
    body: &Bytes,
    request: &RequestHandle,
    response: &mut Parts,
    from_third_party_sdk: bool,
) -> Option<String> {
    // populate the edgee payload from the json
    let payload_result = parse_payload(body.as_ref());
    if payload_result.is_err() {
        warn!("Error parsing json payload: {:?}", payload_result.err());
        return None;
    }
    let mut payload = payload_result.unwrap();

    // prepare the payload for data collection
    payload = prepare_data_collection_payload(payload);

    // add session info
    payload = add_session(request, response, payload);

    // add more info from the request
    payload = add_more_info_from_request(request, payload);

    // populate the events with the data collection context
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .populate_event_contexts();

    let mut events = payload
        .data_collection
        .clone()
        .unwrap()
        .events
        .unwrap_or_default();

    // remove events with all components disabled
    for e in events.clone().iter() {
        if e.is_all_components_disabled() {
            events.retain(|evt| evt.uuid != e.uuid);
        }
    }

    if events.is_empty() {
        return Option::from("[]".to_string());
    }

    let events_json =
        serde_json::to_string(&events).expect("Could not encode data collection events into JSON");
    info!(events = events_json.as_str());

    // send the payload to the data collection components
    tokio::spawn(
        async move {
            if let Err(err) = components::send_data_collection(&events).await {
                warn!(?err, "failed to send data collection payload");
            }
        }
        .in_current_span(),
    );

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

        let b64 = GeneralPurpose::new(&STANDARD, PAD).encode(format!("{}:", api_key));
        let events_json = events_json.clone();
        let debug = if request.is_debug_mode() {
            "true"
        } else {
            "false"
        };
        let from = if from_third_party_sdk {
            "third"
        } else {
            "client"
        };
        let host = request.get_host().to_string();
        // now, we can send the payload to the edgee data-collection-api without waiting for the response
        tokio::spawn(async move {
            let _ = reqwest::Client::new()
                .post(api_url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Basic {}", b64))
                .header("X-Edgee-Debug", debug)
                .header("X-Edgee-From", from)
                .header("X-Edgee-Host", host)
                .body(events_json)
                .send()
                .await;
        });
    }

    Option::from(events_json)
}

/// Prepares the data collection payload by initializing necessary fields if they are not set.
///
/// # Arguments
/// - `payload`: The `Payload` object to be prepared.
///
/// # Returns
/// - `Payload`: The updated `Payload` object with initialized fields.
fn prepare_data_collection_payload(mut payload: Payload) -> Payload {
    // Ensure data_collection and its nested fields are initialized
    payload
        .data_collection
        .get_or_insert_with(Default::default)
        .context
        .get_or_insert_with(Default::default)
        .client
        .get_or_insert_with(Default::default);
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .page
        .get_or_insert_with(Default::default);
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .user
        .get_or_insert_with(Default::default);

    payload
}

/// Adds session information to the payload based on the provided `EdgeeCookie`.
///
/// # Arguments
/// - `request`: A reference to the `RequestHandle` object.
/// - `response`: A mutable reference to the `Parts` object.
/// - `payload`: The `Payload` object to be updated with session information.
///
/// # Returns
/// - `Payload`: The updated `Payload` object with session information.
fn add_session(request: &RequestHandle, response: &mut Parts, mut payload: Payload) -> Payload {
    let edgee_cookie = edgee_cookie::get_or_set(request, response, &payload);

    // edgee_id
    let user_id = edgee_cookie.id.to_string();
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .user
        .as_mut()
        .unwrap()
        .edgee_id = user_id;

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

    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .session = Some(user_session);

    // if edgee_cookie.sz is not None, we add it to the payload
    if edgee_cookie.sz.is_some() {
        if let Some(sz) = &edgee_cookie.sz {
            let size_vec: Vec<&str> = sz.split('x').collect();
            if size_vec.len() == 3 {
                if let (Ok(width), Ok(height), Ok(density)) = (
                    size_vec[0].parse::<i32>(),
                    size_vec[1].parse::<i32>(),
                    size_vec[2].parse::<i32>(),
                ) {
                    payload
                        .data_collection
                        .as_mut()
                        .unwrap()
                        .context
                        .as_mut()
                        .unwrap()
                        .client
                        .as_mut()
                        .unwrap()
                        .screen_width = Some(width);
                    payload
                        .data_collection
                        .as_mut()
                        .unwrap()
                        .context
                        .as_mut()
                        .unwrap()
                        .client
                        .as_mut()
                        .unwrap()
                        .screen_height = Some(height);
                    payload
                        .data_collection
                        .as_mut()
                        .unwrap()
                        .context
                        .as_mut()
                        .unwrap()
                        .client
                        .as_mut()
                        .unwrap()
                        .screen_density = Some(density);
                }
            }
        }
    }

    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .user_agent_architecture = edgee_cookie.uaa.clone();
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .user_agent_bitness = edgee_cookie.uab.clone();
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .user_agent_model = edgee_cookie.uam.clone();
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .os_version = edgee_cookie.uapv.clone();
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .user_agent_full_version_list = edgee_cookie.uafvl.clone();
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .timezone = edgee_cookie.tz.clone();

    payload
}

/// Adds more information from the HTML document or request to the payload.
///
/// # Arguments
/// - `request`: A reference to the `RequestHandle` object.
/// - `document`: A reference to the `Document` object representing the HTML document.
/// - `payload`: The `Payload` object to be updated with additional information.
///
/// # Returns
/// - `Payload`: The updated `Payload` object with additional information from the HTML document or request.
fn add_more_info_from_html_or_request(
    request: &RequestHandle,
    document: &Document,
    mut payload: Payload,
) -> Payload {
    // canonical url
    let mut canonical_url = document.canonical.clone();

    // if canonical is a relative url, we add the domain
    if !canonical_url.is_empty() && !canonical_url.starts_with("http") {
        canonical_url = format!(
            "{}://{}{}",
            request.get_proto(),
            request.get_host(),
            canonical_url
        );
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
        .data_collection
        .as_ref()
        .unwrap()
        .context
        .as_ref()
        .unwrap()
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
        .unwrap_or_else(|| {
            format!(
                "{}://{}{}",
                request.get_proto(),
                request.get_host(),
                request.get_path_and_query().to_string()
            )
        });
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .page
        .as_mut()
        .unwrap()
        .url = Some(url);

    // path: we first try to get it from the payload, then from the canonical, and finally from the request
    let path = payload
        .data_collection
        .as_ref()
        .unwrap()
        .context
        .as_ref()
        .unwrap()
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
        .unwrap_or_else(|| request.get_path_and_query().path().to_string());
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .page
        .as_mut()
        .unwrap()
        .path = Some(path);

    // search: we first try to get it from the payload, and finally from the request
    let search = payload
        .data_collection
        .as_ref()
        .unwrap()
        .context
        .as_ref()
        .unwrap()
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
        .or_else(|| {
            request
                .get_path_and_query()
                .query()
                .map(|qs| "?".to_string() + qs)
        })
        .unwrap_or_default();
    if search == "?" || search.is_empty() {
        // if search is = "?", we leave it blank
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .page
            .as_mut()
            .unwrap()
            .search = None;
    } else {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .page
            .as_mut()
            .unwrap()
            .search = Some(search.clone());
    }

    // title: we first try to get it from the payload, and finally from the title html tag
    let title = payload
        .data_collection
        .as_ref()
        .unwrap()
        .context
        .as_ref()
        .unwrap()
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
        .unwrap_or_default();
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .page
        .as_mut()
        .unwrap()
        .title = Some(title.clone());

    // keywords: we first try to get it from the payload, and finally from the keywords meta tag
    let keywords = payload
        .data_collection
        .as_ref()
        .unwrap()
        .context
        .as_ref()
        .unwrap()
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
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .page
        .as_mut()
        .unwrap()
        .keywords = Some(keywords);

    payload
}

/// Adds more information to the payload from the request headers.
///
/// # Arguments
/// - `request`: A reference to the `RequestHandle` object.
/// - `payload`: The `Payload` object to be updated with additional information.
///
/// # Returns
/// - `Payload`: The updated `Payload` object with additional information from the request headers.
fn add_more_info_from_request(request: &RequestHandle, mut payload: Payload) -> Payload {
    // first, prepare the payload
    if payload
        .data_collection
        .as_ref()
        .unwrap()
        .context
        .as_ref()
        .unwrap()
        .client
        .is_none()
    {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client = Some(Default::default());
    }

    // get referer from request if it is not already in the payload
    if payload
        .data_collection
        .as_ref()
        .unwrap()
        .context
        .as_ref()
        .unwrap()
        .page
        .as_ref()
        .unwrap()
        .referrer
        .is_none()
    {
        let referer = request
            .get_header(header::REFERER)
            .unwrap_or("".to_string());
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .page
            .as_mut()
            .unwrap()
            .referrer = Some(referer);
    }

    // if the referer is empty, we remove it
    if payload
        .data_collection
        .as_ref()
        .unwrap()
        .context
        .as_ref()
        .unwrap()
        .page
        .as_ref()
        .unwrap()
        .referrer
        .as_ref()
        .unwrap()
        .is_empty()
    {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .page
            .as_mut()
            .unwrap()
            .referrer = None;
    }

    // user_agent
    let user_agent = request
        .get_header(header::USER_AGENT)
        .unwrap_or("".to_string());
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .user_agent = Some(user_agent);

    // client ip
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .ip = Some(request.get_client_ip().to_string());

    // anonymize the ip address
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .anonymize_ip();

    // locale
    let locale = get_preferred_language(request.get_headers());
    payload
        .data_collection
        .as_mut()
        .unwrap()
        .context
        .as_mut()
        .unwrap()
        .client
        .as_mut()
        .unwrap()
        .locale = Some(locale);

    // Accept-Language
    let accept_language = request
        .get_header("Accept-Language")
        .unwrap_or("".to_string());
    if !accept_language.is_empty() {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .accept_language = Some(accept_language);
    }

    // sec-ch-ua-arch (user_agent_architecture)
    if let Some(sec_ch_ua_arch) = request.get_header("Sec-Ch-Ua-Arch") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .user_agent_architecture = Some(sec_ch_ua_arch.replace("\"", ""));
    }

    // sec-ch-ua-bitness (user_agent_bitness)
    if let Some(sec_ch_ua_bitness) = request.get_header("Sec-Ch-Ua-Bitness") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .user_agent_bitness = Some(sec_ch_ua_bitness.replace("\"", ""));
    }

    // sec-ch-ua (user_agent_version_list)
    if let Some(sec_ch_ua) = request.get_header("Sec-Ch-Ua") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .user_agent_version_list = Some(process_sec_ch_ua(sec_ch_ua.as_str(), false));

        // if user_agent_full_version_list is not set, we set it to the same value
        if payload
            .data_collection
            .as_ref()
            .unwrap()
            .context
            .as_ref()
            .unwrap()
            .client
            .as_ref()
            .unwrap()
            .user_agent_full_version_list
            .is_none()
        {
            payload
                .data_collection
                .as_mut()
                .unwrap()
                .context
                .as_mut()
                .unwrap()
                .client
                .as_mut()
                .unwrap()
                .user_agent_full_version_list = Some(process_sec_ch_ua(sec_ch_ua.as_str(), true));
        }
    }

    // Sec-Ch-Ua-Full-Version-List (user_agent_full_version_list)
    if let Some(sec_ch_ua) = request.get_header("Sec-Ch-Ua-Full-Version-List") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .user_agent_full_version_list = Some(process_sec_ch_ua(sec_ch_ua.as_str(), true));
    }

    // sec-ch-ua-mobile (user_agent_mobile)
    if let Some(sec_ch_ua_mobile) = request.get_header("Sec-Ch-Ua-Mobile") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .user_agent_mobile = Some(sec_ch_ua_mobile.replace("?", ""));
    }

    // sec-ch-ua-model (user_agent_model)
    if let Some(sec_ch_ua_model) = request.get_header("Sec-Ch-Ua-Model") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .user_agent_model = Some(sec_ch_ua_model.replace("\"", ""));
    }

    // sec-ch-ua-platform (os_name)
    if let Some(sec_ch_ua_platform) = request.get_header("Sec-Ch-Ua-Platform") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .os_name = Some(sec_ch_ua_platform.replace("\"", ""));
    }

    // sec-ch-ua-platform-version (os_version)
    if let Some(sec_ch_ua_platform_version) = request.get_header("Sec-Ch-Ua-Platform-Version") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .client
            .as_mut()
            .unwrap()
            .os_version = Some(sec_ch_ua_platform_version.replace("\"", ""));
    }

    // campaign
    let map: HashMap<String, String> = url::form_urlencoded::parse(request.get_query().as_bytes())
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
    if utm_keys.iter().any(|key| map.contains_key(*key))
        && payload
            .data_collection
            .as_ref()
            .unwrap()
            .context
            .as_ref()
            .unwrap()
            .campaign
            .is_none()
    {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .campaign = Some(Default::default());
    }
    if map.contains_key("utm_campaign") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .campaign
            .as_mut()
            .unwrap()
            .name = Some(map.get("utm_campaign").unwrap().to_string());
    }
    if map.contains_key("utm_source") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .campaign
            .as_mut()
            .unwrap()
            .source = Some(map.get("utm_source").unwrap().to_string());
    }
    if map.contains_key("utm_medium") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .campaign
            .as_mut()
            .unwrap()
            .medium = Some(map.get("utm_medium").unwrap().to_string());
    }
    if map.contains_key("utm_term") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .campaign
            .as_mut()
            .unwrap()
            .term = Some(map.get("utm_term").unwrap().to_string());
    }
    if map.contains_key("utm_content") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .campaign
            .as_mut()
            .unwrap()
            .content = Some(map.get("utm_content").unwrap().to_string());
    }
    if map.contains_key("utm_creative_format") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .campaign
            .as_mut()
            .unwrap()
            .creative_format = Some(map.get("utm_creative_format").unwrap().to_string());
    }
    if map.contains_key("utm_marketing_tactic") {
        payload
            .data_collection
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .campaign
            .as_mut()
            .unwrap()
            .marketing_tactic = Some(map.get("utm_marketing_tactic").unwrap().to_string());
    }

    payload
}

/// Processes the `Sec-CH-UA` header to extract and format the user agent information.
///
/// # Arguments
/// - `header`: A string slice that holds the value of the `Sec-CH-UA` header.
/// - `full`: A boolean flag that indicates whether the full version string should be included.
///
/// # Returns
/// - `String`: A formatted string containing the user agent information, with each key-value pair separated by a semicolon,
///   and multiple pairs separated by a pipe character.
///
/// The function uses a regular expression to capture the key and version from the header.
/// It ensures that the version string has four parts by appending ".0" if necessary.
/// The formatted key-version pairs are then concatenated into a single string.
fn process_sec_ch_ua(header: &str, full: bool) -> String {
    lazy_static::lazy_static! {
        static ref VALUE_REGEX: Regex = Regex::new(r#""([^"]+)";v="([^"]+)""#).unwrap();
    }

    let mut output = String::new();

    let matches: Vec<_> = VALUE_REGEX.captures_iter(header).collect();

    for (i, cap) in matches.iter().enumerate() {
        let key = &cap[1];
        let version = &cap[2];

        // Split the version string into its parts and ensure it has four parts
        let mut parts: Vec<_> = version.split('.').collect();
        while parts.len() < 4 {
            parts.push("0");
        }
        let mut version_str = parts.join(".");
        if !full {
            // get only the major version
            version_str = parts[0].to_string();
        }

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
    serde_json::from_reader(clean_json)
}
