mod context;
mod convert;
mod debug;
pub mod logger;
pub mod payload;
pub mod versions;

use std::str::FromStr;
use std::time::Duration;
use url::Url;

use crate::config::{ComponentsConfiguration, DataCollectionComponents};
use context::EventContext;
use debug::{debug_and_trace_response, trace_disabled_event, trace_request, DebugParams};
use http::{header, HeaderMap, HeaderName, HeaderValue};
use tokio::task::JoinHandle;
use tracing::{error, span, Instrument, Level};

use crate::context::ComponentsContext;

use crate::data_collection::payload::{Consent, Event, EventType};
use std::collections::HashMap;

#[derive(Clone)]
pub struct ComponentMetadata {
    pub component_id: String,
    pub component: String,
    pub anonymization: bool,
}

#[derive(Clone)]
pub struct Response {
    pub status: i32,
    pub body: String,
    pub content_type: String,
    pub message: String,
    pub duration: u128,
}

#[derive(Clone)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub body: String,
    pub headers: HashMap<String, String>,
}

#[derive(Clone)]
pub struct EventResponse {
    pub context: EventContext,
    pub event: Event,
    pub component_metadata: ComponentMetadata,

    pub response: Response,
    pub request: Request,
}

#[derive(Clone)]
pub struct AuthResponse {
    pub component_id: String,
    pub serialized_token_content: String,
    pub token_duration: i64,
    pub component_token_setting_name: String,
}

pub async fn send_json_events(
    component_ctx: &ComponentsContext,
    events_json: &str,
    component_config: &ComponentsConfiguration,
    trace_component: &Option<String>,
    debug: bool,
) -> anyhow::Result<Vec<JoinHandle<EventResponse>>> {
    if events_json.is_empty() {
        return Ok(vec![]);
    }

    let mut events: Vec<Event> = serde_json::from_str(events_json)?;
    send_events(
        component_ctx,
        &mut events,
        component_config,
        trace_component,
        debug,
        "",
        "",
        &HashMap::new(),
    )
    .await
}

pub async fn get_auth_request(
    context: &ComponentsContext,
    component: &DataCollectionComponents,
) -> anyhow::Result<Option<JoinHandle<AuthResponse>>> {
    let mut store = context.empty_store();
    let (headers, method, url, body, auth_metadata) =
        match crate::data_collection::versions::v1_0_1::execute::get_auth_request(
            context, component, &mut store,
        )
        .await
        {
            Ok(Some((headers, method, url, body, auth_metadata))) => {
                (headers, method, url, body, auth_metadata)
            }
            Ok(None) => {
                return Ok(None);
            }
            Err(err) => {
                error!("Failed to get auth request. Error: {}", err);
                return Err(err);
            }
        };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let component_id = component.id.clone();
    let future = tokio::spawn(async move {
        let res = match method.as_str() {
            "GET" => client.get(url.clone()).headers(headers).send().await,
            "PUT" => {
                client
                    .put(url.clone())
                    .headers(headers)
                    .body(body.clone())
                    .send()
                    .await
            }
            "POST" => {
                client
                    .post(url.clone())
                    .headers(headers)
                    .body(body.clone())
                    .send()
                    .await
            }
            _ => {
                return AuthResponse {
                    component_id,
                    serialized_token_content: String::new(),
                    component_token_setting_name: auth_metadata.component_token_setting_name,
                    token_duration: auth_metadata.token_duration,
                }
            }
        };

        let res = res.unwrap();
        let response_text = res.text().await.unwrap_or_default();
        let json_value = serde_json::from_str::<serde_json::Value>(&response_text).ok();

        let expires_in = json_value
            .as_ref()
            .and_then(|json| json.get("expires_in").and_then(|v| v.as_i64()));

        let serialized_token_content = json_value
            .and_then(|json| {
                if auth_metadata.response_token_property_name.is_none() {
                    Some(response_text)
                } else {
                    auth_metadata
                        .response_token_property_name
                        .as_ref()
                        .and_then(|property_name| json.get(property_name))
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                }
            })
            .unwrap_or_else(|| {
                error!("Failed to find token property");
                String::new()
            });

        AuthResponse {
            component_id,
            serialized_token_content,
            component_token_setting_name: auth_metadata.component_token_setting_name,
            token_duration: expires_in.unwrap_or(auth_metadata.token_duration),
        }
    });

    Ok(Some(future))
}

#[allow(clippy::too_many_arguments)]
pub async fn send_events(
    component_ctx: &ComponentsContext,
    events: &mut [Event],
    component_config: &ComponentsConfiguration,
    trace_component: &Option<String>,
    debug: bool,
    project_id: &str,
    proxy_host: &str,
    client_headers: &HashMap<String, String>,
) -> anyhow::Result<Vec<JoinHandle<EventResponse>>> {
    if events.is_empty() {
        return Ok(vec![]);
    }

    let ctx = &EventContext::new(events, project_id, proxy_host);

    let mut store = component_ctx.empty_store();

    let mut futures = vec![];

    // iterate on each event
    for event in events.iter_mut() {
        for cfg in component_config.data_collection.iter() {
            let span = span!(
                Level::INFO,
                "component",
                name = cfg.id.as_str(),
                event = ?event.event_type
            );
            let _enter = span.enter();

            let mut event = event.clone();

            let trace =
                trace_component.is_some() && trace_component.as_ref().unwrap() == cfg.id.as_str();

            if cfg.event_filtering_rules.iter().any(|rule| {
                rule.event_types
                    .iter()
                    .any(|event_type| event_type == &event.event_type.to_string())
                    && rule
                        .conditions
                        .iter()
                        .any(|condition| event.should_filter_out(condition))
            }) {
                trace_disabled_event(trace, "event_filtered");
                continue;
            }

            event.apply_data_manipulation_rules(&cfg.data_manipulation_rules);

            // if event_type is not enabled in config.config.get(component_id).unwrap(), skip the event
            match event.event_type {
                EventType::Page => {
                    if !cfg.settings.edgee_page_event_enabled {
                        trace_disabled_event(trace, "page");
                        continue;
                    }
                }
                EventType::User => {
                    if !cfg.settings.edgee_user_event_enabled {
                        trace_disabled_event(trace, "user");
                        continue;
                    }
                }
                EventType::Track => {
                    if !cfg.settings.edgee_track_event_enabled {
                        trace_disabled_event(trace, "track");
                        continue;
                    }
                }
            }

            if !event.is_component_enabled(cfg) {
                continue;
            }

            let initial_anonymization = cfg.settings.edgee_anonymization;
            let default_consent = cfg.settings.edgee_default_consent.clone();

            // Use the helper function to handle consent and determine anonymization
            let (anonymization, outgoing_consent) = handle_consent_and_anonymization(
                &mut event,
                &default_consent,
                initial_anonymization,
            );

            if anonymization {
                event.context.client.ip = ctx.get_ip_anonymized().clone();
                event.context.page.search = "".to_string();
                event.context.page.referrer = "".to_string();
                event.context.campaign.medium = "".to_string();
                event.context.campaign.name = "".to_string();
                event.context.campaign.source = "".to_string();
                event.context.campaign.content = "".to_string();
                event.context.campaign.creative_format = "".to_string();
                event.context.campaign.marketing_tactic = "".to_string();
                event.context.campaign.term = "".to_string();
            } else {
                event.context.client.ip = ctx.get_ip().clone();
            }

            // Native cookie support
            if let Some(ref ids) = event.context.user.native_cookie_ids {
                if ids.contains_key(&cfg.slug) {
                    event.context.user.edgee_id = ids.get(&cfg.slug).unwrap().clone();
                } else {
                    event.context.user.edgee_id = ctx.get_edgee_id().clone();
                }
            }

            // Add one second to the timestamp if uuid is not the same than the first event, to prevent duplicate sessions
            if &event.uuid != ctx.get_uuid() {
                event.timestamp = *ctx.get_timestamp() + chrono::Duration::seconds(1);
                event.context.session.session_start = false;
            }

            let (headers, method, url, body) = match cfg.wit_version {
                versions::DataCollectionWitVersion::V1_0_0 => {
                    match crate::data_collection::versions::v1_0_0::execute::get_edgee_request(
                        &event,
                        component_ctx,
                        cfg,
                        &mut store,
                        &client_headers.clone(),
                    )
                    .await
                    {
                        Ok((headers, method, url, body)) => (headers, method, url, body),
                        Err(err) => {
                            error!("Failed to get edgee request. Error: {}", err);
                            continue;
                        }
                    }
                }
                versions::DataCollectionWitVersion::V1_0_1 => {
                    match crate::data_collection::versions::v1_0_1::execute::get_edgee_request(
                        &event,
                        component_ctx,
                        cfg,
                        &mut store,
                        &client_headers.clone(),
                    )
                    .await
                    {
                        Ok((headers, method, url, body)) => (headers, method, url, body),
                        Err(err) => {
                            error!("Failed to get edgee request. Error: {}", err);
                            continue;
                        }
                    }
                }
            };

            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?;

            trace_request(
                trace,
                &method,
                &url,
                &headers,
                &body,
                &outgoing_consent,
                anonymization,
            );

            // spawn a separated async thread
            let cfg_project_component_id = cfg.project_component_id.to_string();
            let cfg_id = cfg.id.to_string();
            let ctx_clone = ctx.clone();

            let headers_map = headers.iter().fold(HashMap::new(), |mut acc, (k, v)| {
                acc.insert(k.to_string(), v.to_str().unwrap().to_string());
                acc
            });

            let method_clone = method.to_string();
            let url_clone = url.clone();
            let body_clone = body.clone();

            let future = tokio::spawn(
                async move {
                    let timer = std::time::Instant::now();
                    let res = match method_clone.as_str() {
                        "GET" => client.get(url_clone).headers(headers).send().await,
                        "PUT" => {
                            client
                                .put(url_clone)
                                .headers(headers)
                                .body(body_clone)
                                .send()
                                .await
                        }
                        "POST" => {
                            client
                                .post(url_clone)
                                .headers(headers)
                                .body(body_clone)
                                .send()
                                .await
                        }
                        "DELETE" => client.delete(url_clone).headers(headers).send().await,
                        _ => {
                            return EventResponse {
                                context: ctx_clone,
                                event,
                                component_metadata: ComponentMetadata {
                                    component_id: cfg_project_component_id,
                                    component: cfg_id,
                                    anonymization,
                                },
                                response: Response {
                                    status: 500,
                                    body: "".to_string(),
                                    content_type: "text/plain".to_string(),
                                    message: "Unknown method".to_string(),
                                    duration: timer.elapsed().as_millis(),
                                },
                                request: Request {
                                    method: method_clone,
                                    url: url_clone,
                                    body: body_clone,
                                    headers: headers_map,
                                },
                            }
                        }
                    };

                    let mut debug_params = DebugParams::new(
                        &ctx_clone,
                        &cfg_project_component_id,
                        &cfg_id,
                        &event,
                        &method_clone,
                        &url,
                        &headers_map,
                        &body,
                        timer,
                        anonymization,
                    );

                    let mut message = "".to_string();
                    match res {
                        Ok(res) => {
                            debug_params.response_status =
                                format!("{:?}", res.status()).parse::<i32>().unwrap();

                            debug_params.response_content_type = res
                                .headers()
                                .get("content-type")
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or("text/plain")
                                .to_string();

                            debug_params.response_body = Some(res.text().await.unwrap_or_default());

                            let _r = debug_and_trace_response(
                                debug,
                                trace,
                                debug_params.clone(),
                                "".to_string(),
                            )
                            .await;
                        }
                        Err(err) => {
                            error!(step = "response", status = "500", err = err.to_string());
                            let _r = debug_and_trace_response(
                                debug,
                                trace,
                                debug_params.clone(),
                                err.to_string(),
                            )
                            .await;
                            message = err.to_string();
                        }
                    }

                    EventResponse {
                        context: ctx_clone,
                        event,
                        component_metadata: ComponentMetadata {
                            component_id: cfg_project_component_id,
                            component: cfg_id,
                            anonymization,
                        },
                        response: Response {
                            status: debug_params.response_status,
                            body: debug_params.response_body.unwrap_or_default(),
                            content_type: debug_params.response_content_type,
                            message,
                            duration: timer.elapsed().as_millis(),
                        },
                        request: Request {
                            method,
                            url: url.to_string(),
                            body,
                            headers: headers_map,
                        },
                    }
                }
                .in_current_span(),
            );
            futures.push(future);
        }
    }

    Ok(futures)
}

fn handle_consent_and_anonymization(
    event: &mut Event,
    default_consent: &str,
    initial_anonymization: bool,
) -> (bool, String) {
    // Handle default consent if not set
    if event.consent.is_none() {
        event.consent = match default_consent {
            "granted" => Some(Consent::Granted),
            "denied" => Some(Consent::Denied),
            _ => Some(Consent::Pending),
        };
    }

    let outgoing_consent = event.consent.clone().unwrap().to_string();

    // Determine final anonymization state
    match event.consent {
        Some(Consent::Granted) => (false, outgoing_consent),
        _ => (initial_anonymization, outgoing_consent),
    }
}

pub fn insert_expected_headers(
    headers: &mut HeaderMap,
    event: &Event,
    client_headers: &HashMap<String, String>,
) -> anyhow::Result<()> {
    // Insert client ip in the x-forwarded-for header
    if !event.context.client.ip.is_empty() {
        headers.insert(
            HeaderName::from_str("x-forwarded-for")?,
            HeaderValue::from_str(&event.context.client.ip)?,
        );
    }

    // Insert User-Agent in the user-agent header
    if !event.context.client.user_agent.is_empty() {
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_str(&event.context.client.user_agent)?,
        );
    }

    // Insert referrer in the referer header like an analytics client-side collect does
    if !event.context.page.url.is_empty() {
        let document_location = format!(
            "{}{}",
            event.context.page.url.clone(),
            event.context.page.search.clone()
        );
        headers.insert(
            header::REFERER,
            HeaderValue::from_str(document_location.as_str())?,
        );
    }

    // Insert Accept-Language in the accept-language header
    if !event.context.client.accept_language.is_empty() {
        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_str(event.context.client.accept_language.as_str())?,
        );
    }

    // Insert origin in the origin header
    if let Ok(origin) = get_origin_from_url(&event.context.page.url) {
        if let Ok(value) = HeaderValue::from_str(&origin) {
            headers.insert(header::ORIGIN, value);
        }
    }

    // Insert Accept header
    headers.insert(header::ACCEPT, HeaderValue::from_str("*/*")?);

    // Insert sec-fetch-dest, sec-fetch-mode and sec-fetch-site headers
    headers.insert(
        HeaderName::from_str("sec-fetch-dest")?,
        HeaderValue::from_str("empty")?,
    );
    headers.insert(
        HeaderName::from_str("sec-fetch-mode")?,
        HeaderValue::from_str("no-cors")?,
    );
    headers.insert(
        HeaderName::from_str("sec-fetch-site")?,
        HeaderValue::from_str("cross-site")?,
    );

    // Insert headers from the real client headers
    for (key, value) in client_headers.iter() {
        headers.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
    }

    Ok(())
}

/// Extracts the origin from a URL string.
///
/// The origin consists of the scheme and host components of the URL.
/// For example, given "https://www.example.com/test", this function returns "https://www.example.com".
///
/// # Arguments
///
/// * `url` - A string slice containing the URL to parse
///
/// # Returns
///
/// * `Result<String, anyhow::Error>` - The origin string on success, or an error if URL parsing fails
///
pub fn get_origin_from_url(url: &str) -> Result<String, anyhow::Error> {
    let url = Url::parse(url)?;
    let host = url.host().unwrap().to_string();
    let scheme = url.scheme();
    Ok(format!("{scheme}://{host}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DataCollectionComponentSettings, DataCollectionComponents};
    use crate::data_collection::payload::{Client, Context, Page};
    use http::HeaderValue;

    #[test]
    fn test_get_origin_from_url() {
        // Valid cases
        assert_eq!(
            get_origin_from_url("https://www.example.com/test").unwrap(),
            "https://www.example.com"
        );
        assert_eq!(
            get_origin_from_url("https://de.example.com").unwrap(),
            "https://de.example.com"
        );
        assert_eq!(
            get_origin_from_url("https://www.example.com/test?utm_source=test").unwrap(),
            "https://www.example.com"
        );
        assert_eq!(
            get_origin_from_url("https://www.example.com:8080").unwrap(),
            "https://www.example.com"
        );
        assert_eq!(
            get_origin_from_url("https://www.example.com:8080/test?query=test").unwrap(),
            "https://www.example.com"
        );
        assert_eq!(
            get_origin_from_url("https://www.example.com:8080?query=test").unwrap(),
            "https://www.example.com"
        );
        assert_eq!(
            get_origin_from_url("https://www.example.com:8080/test?query=test").unwrap(),
            "https://www.example.com"
        );

        // Edge cases
        assert!(get_origin_from_url("").is_err());
        assert!(get_origin_from_url("Invalid").is_err());
        assert!(get_origin_from_url("No Version;").is_err());
        assert!(get_origin_from_url(";No Brand").is_err());
    }

    fn create_test_event() -> Event {
        Event {
            context: Context {
                client: Client {
                    ip: "192.168.1.1".to_string(),
                    user_agent: "Mozilla/5.0".to_string(),
                    accept_language: "en-US,en;q=0.9".to_string(),
                    user_agent_version_list: "Chromium;128|Google Chrome;128".to_string(),
                    user_agent_mobile: "0".to_string(),
                    os_name: "Windows".to_string(),
                    ..Default::default()
                },
                page: Page {
                    url: "https://example.com".to_string(),
                    search: "?query=test".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn create_empty_test_event() -> Event {
        Event {
            context: Context {
                client: Client {
                    ip: "".to_string(),
                    user_agent: "".to_string(),
                    accept_language: "".to_string(),
                    user_agent_version_list: "".to_string(),
                    user_agent_mobile: "".to_string(),
                    os_name: "".to_string(),
                    ..Default::default()
                },
                page: Page {
                    url: "".to_string(),
                    search: "".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_insert_expected_headers() {
        let mut headers = HeaderMap::new();
        let event = create_test_event();

        let mut client_headers: HashMap<String, String> = HashMap::new();
        client_headers.insert(
            "sec-ch-ua".to_string(),
            "\"Chromium\";v=\"128\", \"Google Chrome\";v=\"128\"".to_string(),
        );
        client_headers.insert("sec-ch-ua-mobile".to_string(), "?0".to_string());
        client_headers.insert("sec-ch-ua-platform".to_string(), "\"Windows\"".to_string());

        let result = insert_expected_headers(&mut headers, &event, &client_headers);

        assert!(result.is_ok());
        assert_eq!(
            headers.get("x-forwarded-for"),
            Some(&HeaderValue::from_str("192.168.1.1").unwrap())
        );
        assert_eq!(
            headers.get("user-agent"),
            Some(&HeaderValue::from_str("Mozilla/5.0").unwrap())
        );
        assert_eq!(
            headers.get("accept-language"),
            Some(&HeaderValue::from_str("en-US,en;q=0.9").unwrap())
        );
        assert_eq!(
            headers.get("referer"),
            Some(&HeaderValue::from_str("https://example.com?query=test").unwrap())
        );
        assert_eq!(
            headers.get("sec-ch-ua"),
            Some(
                &HeaderValue::from_str("\"Chromium\";v=\"128\", \"Google Chrome\";v=\"128\"")
                    .unwrap()
            )
        );
        assert_eq!(
            headers.get("sec-ch-ua-mobile"),
            Some(&HeaderValue::from_str("?0").unwrap())
        );
        assert_eq!(
            headers.get("sec-ch-ua-platform"),
            Some(&HeaderValue::from_str("\"Windows\"").unwrap())
        );
    }

    #[test]
    fn test_insert_expected_headers_with_empty_fields() {
        let mut headers = HeaderMap::new();

        let event = create_empty_test_event();

        // Call the function
        let result = insert_expected_headers(&mut headers, &event, &HashMap::new());

        assert!(result.is_ok());
        assert_eq!(headers.keys().len(), 4);
    }

    #[test]
    fn test_handle_consent_and_anonymization_granted() {
        let mut event = Event {
            consent: None,
            ..Default::default()
        };

        // Test with default consent "granted"
        let (anonymization, outgoing_consent) =
            handle_consent_and_anonymization(&mut event, "granted", true);
        assert_eq!(event.consent, Some(Consent::Granted));
        assert!(!anonymization);
        assert_eq!(outgoing_consent, "granted");
    }

    #[test]
    fn test_handle_consent_and_anonymization_denied() {
        // Test with default consent "denied"
        let mut event = Event {
            consent: None,
            ..Default::default()
        };
        let (anonymization, outgoing_consent) =
            handle_consent_and_anonymization(&mut event, "denied", true);
        assert_eq!(event.consent, Some(Consent::Denied));
        assert!(anonymization);
        assert_eq!(outgoing_consent, "denied");
    }

    #[test]
    fn test_handle_consent_and_anonymization_pending() {
        // Test with default consent "pending"
        let mut event = Event {
            consent: None,
            ..Default::default()
        };
        let (anonymization, outgoing_consent) =
            handle_consent_and_anonymization(&mut event, "pending", true);
        assert_eq!(event.consent, Some(Consent::Pending));
        assert!(anonymization);
        assert_eq!(outgoing_consent, "pending");
    }

    #[test]
    fn test_handle_consent_and_anonymization_existing_granted() {
        // Test with existing consent "granted"
        let mut event = Event {
            consent: Some(Consent::Granted),
            ..Default::default()
        };
        let (anonymization, outgoing_consent) =
            handle_consent_and_anonymization(&mut event, "denied", true);
        assert_eq!(event.consent, Some(Consent::Granted));
        assert!(!anonymization);
        assert_eq!(outgoing_consent, "granted");
    }

    #[test]
    fn test_handle_consent_and_anonymization_existing_denied() {
        // Test with existing consent "denied"
        let mut event = Event {
            consent: Some(Consent::Denied),
            ..Default::default()
        };
        let (anonymization, outgoing_consent) =
            handle_consent_and_anonymization(&mut event, "granted", false);
        assert_eq!(event.consent, Some(Consent::Denied));
        assert!(!anonymization);
        assert_eq!(outgoing_consent, "denied");
    }

    #[test]
    fn test_handle_consent_and_anonymization_existing_pending() {
        // Test with existing consent "pending"
        let mut event = Event {
            consent: Some(Consent::Pending),
            ..Default::default()
        };
        let (anonymization, outgoing_consent) =
            handle_consent_and_anonymization(&mut event, "granted", true);
        assert_eq!(event.consent, Some(Consent::Pending));
        assert!(anonymization);
        assert_eq!(outgoing_consent, "pending");
    }

    #[tokio::test]
    async fn test_send_json_events_with_empty_json() {
        let component_config = ComponentsConfiguration::default();
        let component_ctx = ComponentsContext::new(&component_config).unwrap();
        let events_json = "";
        let trace_component = None;
        let debug = false;

        let result = send_json_events(
            &component_ctx,
            events_json,
            &component_config,
            &trace_component,
            debug,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    fn create_sample_json_events() -> String {
        r#"[{
            "context": {
                "client": {
                    "ip": "192.168.1.1",
                    "user_agent": "Mozilla/5.0",
                    "accept_language": "en-US,en;q=0.9",
                    "user_agent_version_list": "Chromium;128|Google Chrome;128",
                    "user_agent_mobile": "0",
                    "os_name": "Windows"
                },
                "session": {
                    "session_id": "12345",
                    "previous_session_id": "67890",
                    "session_start": true,
                    "session_count": 123,
                    "first_seen": "2023-01-01T00:00:00Z",
                    "last_seen": "2023-01-01T00:00:00Z"
                },
                "page": {
                    "title": "Test Page",
                    "referrer": "https://example.com",
                    "path": "/test",
                    "url": "https://example.com/test",
                    "search": "?query=test"
                },
                "user": {
                    "edgee_id": "abc123"
                }
            },
            "data": {
                "title": "Test Page",
                "referrer": "https://example.com",
                "path": "/test",
                "url": "https://example.com/test",
                "search": "?query=test"
            },
            "type": "page",
            "uuid": "12345",
            "timestamp": "2025-01-01T00:00:00Z",
            "consent": "granted"
        }]"#
        .to_string()
    }

    fn create_component_config() -> ComponentsConfiguration {
        let mut component_config = ComponentsConfiguration::default();
        component_config
            .data_collection
            .push(DataCollectionComponents {
                id: "test_component".to_string(),
                slug: "test_slug".to_string(),
                file: String::from("tests/ga.wasm"),
                project_component_id: "test_project_component_id".to_string(),
                settings: DataCollectionComponentSettings {
                    edgee_page_event_enabled: true,
                    edgee_user_event_enabled: true,
                    edgee_track_event_enabled: true,
                    edgee_anonymization: true,
                    edgee_default_consent: "granted".to_string(),
                    additional_settings: {
                        let mut map = HashMap::new();
                        map.insert("ga_measurement_id".to_string(), "abcdefg".to_string());
                        map
                    },
                },
                wit_version: versions::DataCollectionWitVersion::V1_0_0,
                event_filtering_rules: vec![],
                data_manipulation_rules: vec![],
            });
        component_config
    }

    #[tokio::test]
    async fn test_send_json_events_with_single_event() {
        let component_config = create_component_config();
        let ctx = ComponentsContext::new(&component_config).unwrap();

        let result = send_json_events(
            &ctx,
            create_sample_json_events().as_str(),
            &component_config,
            &None,
            false,
        )
        .await;

        let handles = result.unwrap_or_else(|err| {
            println!("Error: {:?}", err);
            panic!("Test failed");
        });
        assert_eq!(handles.len(), 1);

        // verify the future's result
        let event_response = handles.into_iter().next().unwrap().await.unwrap();
        assert_eq!(event_response.event.event_type, EventType::Page);
        assert_eq!(event_response.event.context.client.ip, "192.168.1.1");
        assert_eq!(
            event_response.event.context.page.url,
            "https://example.com/test"
        );
        assert_eq!(event_response.event.consent, Some(Consent::Granted));
    }
}
