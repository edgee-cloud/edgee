mod convert;
mod debug;
pub mod logger;
pub mod payload;

wasmtime::component::bindgen!({
    world: "data-collection",
    path: "wit/",
    async: true,
});

use std::str::FromStr;
use std::time::Duration;

use crate::config::ComponentsConfiguration;
use chrono::{DateTime, Utc};
use debug::{debug_and_trace_response, trace_disabled_event, trace_request, DebugParams};
use http::{header, HeaderMap, HeaderName, HeaderValue};
use tracing::{error, span, Instrument, Level};

use crate::{
    data_collection::exports::edgee::protocols::data_collection as Component,
    data_collection::payload::{Consent, Event, EventType},
};

use crate::context::ComponentsContext;

pub async fn send_json_events(
    ctx: &ComponentsContext,
    events_json: &str,
    component_config: &ComponentsConfiguration,
    trace_component: &Option<String>,
    debug: bool,
) -> anyhow::Result<()> {
    if events_json.is_empty() {
        return Ok(());
    }

    let mut events: Vec<Event> = serde_json::from_str(events_json)?;
    send_events(
        ctx,
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
    ctx: &ComponentsContext,
    events: &mut [Event],
    component_config: &ComponentsConfiguration,
    trace_component: &Option<String>,
    debug: bool,
    project_id: &str,
    proxy_host: &str,
) -> anyhow::Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let request_info = RequestInfo::new(events, project_id, proxy_host);

    let mut store = ctx.empty_store();

    // iterate on each event
    for event in events.iter_mut() {
        for cfg in component_config.data_collection.iter() {
            let span = span!(
                Level::INFO,
                "component",
                name = cfg.name.as_str(),
                event = ?event.event_type
            );
            let _enter = span.enter();

            let mut event = event.clone();

            let trace =
                trace_component.is_some() && trace_component.as_ref().unwrap() == cfg.name.as_str();

            // if event_type is not enabled in config.config.get(component_id).unwrap(), skip the event
            match event.event_type {
                EventType::Page => {
                    if !cfg.config.page_event_enabled {
                        trace_disabled_event(trace, "page");
                        continue;
                    }
                }
                EventType::User => {
                    if !cfg.config.user_event_enabled {
                        trace_disabled_event(trace, "user");
                        continue;
                    }
                }
                EventType::Track => {
                    if !cfg.config.track_event_enabled {
                        trace_disabled_event(trace, "track");
                        continue;
                    }
                }
            }

            if !event.is_component_enabled(&cfg.name) {
                continue;
            }

            let initial_anonymization = cfg.config.anonymization;
            let default_consent = cfg.config.default_consent.clone();
            let incoming_consent = request_info.consent.clone();
            let _path = event.context.page.path.clone();
            let _event_type = match event.event_type {
                EventType::Page => "page",
                EventType::Track => "track",
                EventType::User => "user",
            };

            // Use the helper function to handle consent and determine anonymization
            let (anonymization, outgoing_consent) = handle_consent_and_anonymization(
                &mut event,
                &default_consent,
                initial_anonymization,
            );

            if anonymization {
                event.context.client.ip = request_info.ip_anonymized.clone();
                // todo: anonymize other data, utm, referrer, etc.
            } else {
                event.context.client.ip = request_info.ip.clone();
            }

            // Native cookie support
            if let Some(ref ids) = event.context.user.native_cookie_ids {
                if ids.contains_key(&cfg.name) {
                    event.context.user.edgee_id = ids.get(&cfg.name).unwrap().clone();
                } else {
                    event.context.user.edgee_id = request_info.edgee_id.clone();
                }
            }

            // Add one second to the timestamp if uuid is not the same than the first event, to prevent duplicate sessions
            if event.uuid != request_info.uuid {
                event.timestamp = request_info.timestamp + chrono::Duration::seconds(1);
                event.context.session.session_start = false;
            }

            // get the instance of the component
            let instance = ctx
                .get_data_collection_instance(&cfg.name, &mut store)
                .await?;
            let component = instance.edgee_protocols_data_collection();

            let component_event: Component::Event = event.clone().into();
            let credentials: Vec<(String, String)> = cfg.credentials.clone().into_iter().collect();

            // call the corresponding method of the component
            let request = match component_event.event_type {
                Component::EventType::Page => {
                    component
                        .call_page(&mut store, &component_event, &credentials)
                        .await
                }
                Component::EventType::Track => {
                    component
                        .call_track(&mut store, &component_event, &credentials)
                        .await
                }
                Component::EventType::User => {
                    component
                        .call_user(&mut store, &component_event, &credentials)
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
            insert_expected_headers(&mut headers, &event, &component_event)?;

            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?;

            trace_request(trace, &request, &headers, &outgoing_consent, anonymization);

            // spawn a separated async thread
            let cfg_id = cfg.id.to_string();
            let cfg_name = cfg.name.to_string();
            let request_info_from = request_info.from.clone();
            let project_id = project_id.to_string();

            tokio::spawn(
                async move {
                    let timer = std::time::Instant::now();
                    let request_clone = request.clone();
                    let res = match request.method {
                        Component::HttpMethod::Get => {
                            client.get(request.url).headers(headers).send().await
                        }
                        Component::HttpMethod::Put => {
                            client
                                .put(request.url)
                                .headers(headers)
                                .body(request.body)
                                .send()
                                .await
                        }
                        Component::HttpMethod::Post => {
                            client
                                .post(request.url)
                                .headers(headers)
                                .body(request.body)
                                .send()
                                .await
                        }
                        Component::HttpMethod::Delete => {
                            client.delete(request.url).headers(headers).send().await
                        }
                    };

                    let mut debug_params = DebugParams {
                        from: &request_info_from,
                        project_id: &project_id,
                        component_id: &cfg_id,
                        component_slug: &cfg_name,
                        event: &event,
                        request: &request_clone,
                        response_content_type: "",
                        response_status: 500,
                        response_body: None,
                        timer,
                        anonymization,
                        incoming_consent: &incoming_consent,
                    };

                    match res {
                        Ok(res) => {
                            let status_str = format!("{:?}", res.status());
                            debug_params.response_status = status_str.parse::<i32>().unwrap();

                            let content_type = res
                                .headers()
                                .get("content-type")
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or("text/plain")
                                .to_string();
                            debug_params.response_content_type = &content_type;

                            let body = res.text().await.unwrap_or_default();
                            debug_params.response_body = Some(body.clone());

                            let _r = debug_and_trace_response(
                                debug,
                                trace,
                                debug_params,
                                "".to_string(),
                            )
                            .await;
                        }
                        Err(err) => {
                            error!(step = "response", status = "500", err = err.to_string());
                            let _r = debug_and_trace_response(
                                debug,
                                trace,
                                debug_params,
                                err.to_string(),
                            )
                            .await;
                        }
                    }
                }
                .in_current_span(),
            );
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct RequestInfo {
    pub from: String,
    pub ip: String,
    pub ip_anonymized: String,
    pub consent: String,
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
    pub edgee_id: String,
    pub proxy_type: String,
    pub proxy_desc: String,
    pub as_name: String,
    pub project_id: String,
    pub proxy_host: String,
}

impl RequestInfo {
    pub fn new(events: &[Event], project_id: &str, proxy_host: &str) -> Self {
        let mut request_info = RequestInfo {
            from: "-".to_string(),
            ip: "".to_string(),
            ip_anonymized: "".to_string(),
            consent: "default".to_string(),
            uuid: "".to_string(),
            timestamp: chrono::Utc::now(),
            edgee_id: "".to_string(),
            proxy_type: "".to_string(),
            proxy_desc: "".to_string(),
            as_name: "".to_string(),
            project_id: project_id.to_string(),
            proxy_host: proxy_host.to_string(),
        };
        if let Some(event) = events.first() {
            // set request_info from the first event
            request_info.from = event.from.clone().unwrap_or("-".to_string());
            request_info.ip = event.context.client.ip.clone();
            request_info.ip_anonymized = anonymize_ip(request_info.ip.clone());
            if event.consent.is_some() {
                request_info.consent = event.consent.as_ref().unwrap().to_string();
            }
            request_info.uuid = event.uuid.clone();
            request_info.timestamp = event.timestamp;
            request_info.edgee_id = event.context.user.edgee_id.clone();
            request_info.proxy_type = event
                .context
                .client
                .proxy_type
                .clone()
                .unwrap_or("-".to_string());
            request_info.proxy_desc = event
                .context
                .client
                .proxy_desc
                .clone()
                .unwrap_or("-".to_string());
            request_info.as_name = event
                .context
                .client
                .as_name
                .clone()
                .unwrap_or("-".to_string());
        }
        request_info
    }
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

fn anonymize_ip(ip: String) -> String {
    if ip.is_empty() {
        return ip;
    }

    use std::net::IpAddr;

    const KEEP_IPV4_BYTES: usize = 3;
    const KEEP_IPV6_BYTES: usize = 6;

    let ip: IpAddr = ip.clone().parse().unwrap();
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

    anonymized_ip.to_string()
}

fn insert_expected_headers(
    headers: &mut HeaderMap,
    event: &Event,
    data_collection_event: &Component::Event,
) -> anyhow::Result<()> {
    // Insert client ip in the x-forwarded-for header
    headers.insert(
        HeaderName::from_str("x-forwarded-for")?,
        HeaderValue::from_str(&data_collection_event.context.client.ip)?,
    );

    // Insert User-Agent in the user-agent header
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_str(&data_collection_event.context.client.user_agent)?,
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
