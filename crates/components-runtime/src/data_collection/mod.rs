mod context;
mod debug;
pub mod logger;
pub mod payload;
pub mod version;

wasmtime::component::bindgen!({
    world: "data-collection",
    path: "wit/",
    async: true,
});

use std::str::FromStr;
use std::time::Duration;

use crate::config::ComponentsConfiguration;
use context::EventContext;
use debug::{debug_and_trace_response, trace_disabled_event, trace_request, DebugParams};
use http::{header, HeaderMap, HeaderName, HeaderValue};
use tokio::task::JoinHandle;
use tracing::{error, span, Instrument, Level};

use crate::context::ComponentsContext;

use crate::{
    data_collection::exports::edgee::components0_5_0::data_collection as Component0_5_0,
    data_collection::exports::edgee::components1_0_0::data_collection as Component1_0_0,
    data_collection::payload::{Consent, Event, EventType},
};
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
    )
    .await
}

pub async fn send_events(
    component_ctx: &ComponentsContext,
    events: &mut [Event],
    component_config: &ComponentsConfiguration,
    trace_component: &Option<String>,
    debug: bool,
    project_id: &str,
    proxy_host: &str,
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
                // todo: anonymize other data, utm, referrer, etc.
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

            // get the instance of the component
            let instance = match component_ctx
                .get_data_collection_instance(&cfg.id, &mut store)
                .await
            {
                Ok(instance) => instance,
                Err(err) => {
                    error!("Failed to get data collection instance. Error: {}", err);
                    continue;
                }
            };

            let (headers, method, url, body) = match cfg.version {
                version::DataCollectionProtocolVersion::V0_5_0 => {
                    let component = instance.edgee_components0_5_0_data_collection();

                    let component_settings: Vec<(String, String)> = cfg
                        .settings
                        .additional_settings
                        .clone()
                        .into_iter()
                        .collect();

                    // call the corresponding method of the component
                    let request = match event.event_type {
                        EventType::Page => {
                            component
                                .call_page(&mut store, &event.clone().into(), &component_settings)
                                .await
                        }
                        EventType::Track => {
                            component
                                .call_track(&mut store, &event.clone().into(), &component_settings)
                                .await
                        }
                        EventType::User => {
                            component
                                .call_user(&mut store, &event.clone().into(), &component_settings)
                                .await
                        }
                    };
                    let request = match request {
                        Ok(Ok(request)) => request,
                        Ok(Err(err)) => {
                            // todo: debug and trace response (error)
                            error!(
                                step = "request",
                                err = err.to_string(),
                                "failed to handle data collection payload"
                            );
                            continue;
                        }
                        Err(err) => {
                            // todo: debug and trace response (error)
                            error!(
                                step = "request",
                                err = err.to_string(),
                                "failed to handle data collection payload"
                            );
                            continue;
                        }
                    };

                    let mut headers = HeaderMap::new();
                    for (key, value) in request.headers.iter() {
                        headers.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
                    }

                    if request.forward_client_headers {
                        insert_expected_headers(&mut headers, &event)?;
                    }

                    let method = match request.method {
                        Component0_5_0::HttpMethod::Get => "GET",
                        Component0_5_0::HttpMethod::Put => "PUT",
                        Component0_5_0::HttpMethod::Post => "POST",
                        Component0_5_0::HttpMethod::Delete => "DELETE",
                    }
                    .to_string();

                    (headers, method, request.url, request.body)
                }
                version::DataCollectionProtocolVersion::V1_0_0 => {
                    let component = instance.edgee_components1_0_0_data_collection();

                    let component_settings: Vec<(String, String)> = cfg
                        .settings
                        .additional_settings
                        .clone()
                        .into_iter()
                        .collect();

                    // call the corresponding method of the component
                    let request = match event.event_type {
                        EventType::Page => {
                            component
                                .call_page(&mut store, &event.clone().into(), &component_settings)
                                .await
                        }
                        EventType::Track => {
                            component
                                .call_track(&mut store, &event.clone().into(), &component_settings)
                                .await
                        }
                        EventType::User => {
                            component
                                .call_user(&mut store, &event.clone().into(), &component_settings)
                                .await
                        }
                    };
                    let request = match request {
                        Ok(Ok(request)) => request,
                        Ok(Err(err)) => {
                            // todo: debug and trace response (error)
                            error!(
                                step = "request",
                                err = err.to_string(),
                                "failed to handle data collection payload"
                            );
                            continue;
                        }
                        Err(err) => {
                            // todo: debug and trace response (error)
                            error!(
                                step = "request",
                                err = err.to_string(),
                                "failed to handle data collection payload"
                            );
                            continue;
                        }
                    };

                    let mut headers = HeaderMap::new();
                    for (key, value) in request.headers.iter() {
                        headers.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
                    }

                    if request.forward_client_headers {
                        insert_expected_headers(&mut headers, &event)?;
                    }

                    let method = match request.method {
                        Component1_0_0::HttpMethod::Get => "GET",
                        Component1_0_0::HttpMethod::Put => "PUT",
                        Component1_0_0::HttpMethod::Post => "POST",
                        Component1_0_0::HttpMethod::Delete => "DELETE",
                    }
                    .to_string();

                    (headers, method, request.url, request.body)
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

pub fn insert_expected_headers(headers: &mut HeaderMap, event: &Event) -> anyhow::Result<()> {
    // Insert client ip in the x-forwarded-for header
    headers.insert(
        HeaderName::from_str("x-forwarded-for")?,
        HeaderValue::from_str(&event.context.client.ip)?,
    );

    // Insert User-Agent in the user-agent header
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_str(&event.context.client.user_agent)?,
    );

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

    // Insert sec-ch-ua headers
    // sec-ch-ua
    if !event.context.client.user_agent_version_list.is_empty() {
        let ch_ua_value = format_ch_ua_header(&event.context.client.user_agent_version_list);
        headers.insert(
            HeaderName::from_str("sec-ch-ua")?,
            HeaderValue::from_str(ch_ua_value.as_str())?,
        );
    }
    // sec-ch-ua-mobile
    if !event.context.client.user_agent_mobile.is_empty() {
        let mobile_value = format!("?{}", event.context.client.user_agent_mobile.clone());
        headers.insert(
            HeaderName::from_str("sec-ch-ua-mobile")?,
            HeaderValue::from_str(mobile_value.as_str())?,
        );
    }
    // sec-ch-ua-platform
    if !event.context.client.os_name.is_empty() {
        let platform_value = format!("\"{}\"", event.context.client.os_name.clone());
        headers.insert(
            HeaderName::from_str("sec-ch-ua-platform")?,
            HeaderValue::from_str(platform_value.as_str())?,
        );
    }

    Ok(())
}

fn format_ch_ua_header(string: &str) -> String {
    if string.is_empty() {
        return String::new();
    }

    let mut ch_ua_list = vec![];

    // Split into individual brand-version pairs
    let pairs = if string.contains('|') {
        string.split('|').collect::<Vec<_>>()
    } else {
        vec![string]
    };

    // Process each pair
    for pair in pairs {
        if let Some((brand, version)) = parse_brand_version(pair) {
            ch_ua_list.push(format!("\"{}\";v=\"{}\"", brand, version));
        }
    }

    ch_ua_list.join(", ")
}

// Helper function to parse a single brand-version pair
fn parse_brand_version(pair: &str) -> Option<(String, &str)> {
    if !pair.contains(';') {
        return None;
    }

    let parts: Vec<&str> = pair.split(';').collect();
    if parts.len() < 2 {
        return None;
    }

    // brand is everything except the last part
    let brand = parts[0..parts.len() - 1].join(";");
    // version is the last part
    let version = parts[parts.len() - 1];

    if brand.is_empty() || version.is_empty() {
        return None;
    }

    Some((brand, version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ch_ua_header() {
        // Valid cases
        assert_eq!(
            format_ch_ua_header("Chromium;128"),
            "\"Chromium\";v=\"128\""
        );
        assert_eq!(
            format_ch_ua_header("Chromium;128|Google Chrome;128"),
            "\"Chromium\";v=\"128\", \"Google Chrome\";v=\"128\""
        );
        assert_eq!(
            format_ch_ua_header("Not;A=Brand;24"),
            "\"Not;A=Brand\";v=\"24\""
        );
        assert_eq!(
            format_ch_ua_header("Chromium;128|Google Chrome;128|Not;A=Brand;24"),
            "\"Chromium\";v=\"128\", \"Google Chrome\";v=\"128\", \"Not;A=Brand\";v=\"24\""
        );
        assert_eq!(
            format_ch_ua_header("Chromium;128|Google Chrome;128|Not_A Brand;24|Opera;128"),
            "\"Chromium\";v=\"128\", \"Google Chrome\";v=\"128\", \"Not_A Brand\";v=\"24\", \"Opera\";v=\"128\""
        );

        // Edge cases
        assert_eq!(format_ch_ua_header(""), "");
        assert_eq!(format_ch_ua_header("Invalid"), "");
        assert_eq!(format_ch_ua_header("No Version;"), "");
        assert_eq!(format_ch_ua_header(";No Brand"), "");
    }
}
