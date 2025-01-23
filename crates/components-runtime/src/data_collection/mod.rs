mod convert;
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
use http::{header, HeaderMap, HeaderName, HeaderValue};
use json_pretty::PrettyFormatter;
use tracing::{error, info, span, Instrument, Level};

use crate::{
    data_collection::exports::edgee::protocols::data_collection::{self},
    data_collection::payload::{Consent, Event, EventType},
};

use crate::context::ComponentsContext;

pub async fn send_events(
    ctx: &ComponentsContext,
    events_json: &str,
    component_config: &ComponentsConfiguration,
    log_component: &Option<String>,
) -> anyhow::Result<()> {
    if events_json.is_empty() {
        return Ok(());
    }

    let mut events: Vec<Event> = serde_json::from_str(events_json)?;

    if events.is_empty() {
        return Ok(());
    }

    let request_info = RequestInfo::new(&events);

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

            let debug =
                log_component.is_some() && log_component.as_ref().unwrap() == cfg.name.as_str();

            // if event_type is not enabled in config.config.get(component_id).unwrap(), skip the event
            match event.event_type {
                EventType::Page => {
                    if !cfg.config.page_event_enabled {
                        debug_disabled_event(debug, "page");
                        continue;
                    }
                }
                EventType::User => {
                    if !cfg.config.user_event_enabled {
                        debug_disabled_event(debug, "user");
                        continue;
                    }
                }
                EventType::Track => {
                    if !cfg.config.track_event_enabled {
                        debug_disabled_event(debug, "track");
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

            let component_event: data_collection::Event = event.clone().into();
            let credentials: Vec<(String, String)> = cfg.credentials.clone().into_iter().collect();

            // call the corresponding method of the component
            let request = match component_event.event_type {
                data_collection::EventType::Page => {
                    component
                        .call_page(&mut store, &component_event, &credentials)
                        .await
                }
                data_collection::EventType::Track => {
                    component
                        .call_track(&mut store, &component_event, &credentials)
                        .await
                }
                data_collection::EventType::User => {
                    component
                        .call_user(&mut store, &component_event, &credentials)
                        .await
                }
            };
            let request = match request {
                Ok(Ok(request)) => request,
                Ok(Err(err)) => {
                    error!(
                        step = "request",
                        err = err.to_string(),
                        "failed to handle data collection payload"
                    );
                    continue;
                }
                Err(err) => {
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
            // let client = client.clone();

            let method_str = match request.method {
                data_collection::HttpMethod::Get => "GET",
                data_collection::HttpMethod::Put => "PUT",
                data_collection::HttpMethod::Post => "POST",
                data_collection::HttpMethod::Delete => "DELETE",
            };

            info!(
                step = "request",
                method = method_str,
                url = request.url,
                body = request.body
            );

            debug_request(
                debug,
                &request,
                &headers,
                incoming_consent,
                outgoing_consent,
                anonymization,
            );

            // spawn a separated async thread
            tokio::spawn(
                async move {
                    let timer_start = std::time::Instant::now();
                    let res = match request.method {
                        data_collection::HttpMethod::Get => {
                            client.get(request.url).headers(headers).send().await
                        }
                        data_collection::HttpMethod::Put => {
                            client
                                .put(request.url)
                                .headers(headers)
                                .body(request.body)
                                .send()
                                .await
                        }
                        data_collection::HttpMethod::Post => {
                            client
                                .post(request.url)
                                .headers(headers)
                                .body(request.body)
                                .send()
                                .await
                        }
                        data_collection::HttpMethod::Delete => {
                            client.delete(request.url).headers(headers).send().await
                        }
                    };

                    match res {
                        Ok(res) => {
                            let is_success = res.status().is_success();
                            let status_str = format!("{:?}", res.status());
                            let body_res_str = res.text().await.unwrap_or_default();

                            if is_success {
                                info!(step = "response", status = status_str, body = body_res_str);
                            } else {
                                error!(step = "response", status = status_str, body = body_res_str);
                            }
                            debug_response(
                                debug,
                                &status_str,
                                timer_start,
                                body_res_str,
                                "".to_string(),
                            );
                        }
                        Err(err) => {
                            error!(step = "response", status = "500", err = err.to_string());
                            debug_response(
                                debug,
                                "502",
                                timer_start,
                                "".to_string(),
                                err.to_string(),
                            );
                        }
                    }
                }
                .in_current_span(),
            );
        }
    }
    Ok(())
}

pub struct RequestInfo {
    pub from: String,
    pub ip: String,
    pub ip_anonymized: String,
    pub consent: String,
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
}

impl RequestInfo {
    pub fn new(events: &[Event]) -> Self {
        let mut request_info = RequestInfo {
            from: "-".to_string(),
            ip: "".to_string(),
            ip_anonymized: "".to_string(),
            consent: "default".to_string(),
            uuid: "".to_string(),
            timestamp: chrono::Utc::now(),
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
    data_collection_event: &data_collection::Event,
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

fn debug_disabled_event(debug: bool, event: &str) {
    if !debug {
        return;
    }

    println!("--------------------------------------------");
    println!(" Event {} is disabled for this component", event);
    println!("--------------------------------------------\n");
}

fn debug_request(
    debug: bool,
    request: &data_collection::EdgeeRequest,
    headers: &HeaderMap,
    _incoming_consent: String,
    outgoing_consent: String,
    anonymization: bool,
) {
    if !debug {
        return;
    }

    let method_str = match request.method {
        data_collection::HttpMethod::Get => "GET",
        data_collection::HttpMethod::Put => "PUT",
        data_collection::HttpMethod::Post => "POST",
        data_collection::HttpMethod::Delete => "DELETE",
    };

    let anonymization_str = if anonymization { "true" } else { "false" };

    println!("-----------");
    println!("  REQUEST  ");
    println!("-----------\n");
    println!(
        "Config:   Consent: {}, Anonymization: {}",
        outgoing_consent, anonymization_str
    );
    println!("Method:   {}", method_str);
    println!("Url:      {}", request.url);
    if !headers.is_empty() {
        print!("Headers:  ");
        for (i, (key, value)) in headers.iter().enumerate() {
            if i == 0 {
                println!("{}: {:?}", key, value);
            } else {
                println!("          {}: {:?}", key, value);
            }
        }
    } else {
        println!("Headers:  None");
    }

    if !request.body.is_empty() {
        println!("Body:");
        let formatter = PrettyFormatter::from_str(request.body.as_str());
        let result = formatter.pretty();
        println!("{}", result);
    } else {
        println!("Body:     None");
    }
    println!();
}

fn debug_response(
    debug: bool,
    status: &str,
    timer_start: std::time::Instant,
    body: String,
    error: String,
) {
    if !debug {
        return;
    }

    println!("------------");
    println!("  RESPONSE  ");
    println!("------------\n");
    println!("Status:   {}", status);
    println!("Duration: {}ms", timer_start.elapsed().as_millis());
    if !body.is_empty() {
        println!("Body:");
        let formatter = PrettyFormatter::from_str(body.as_str());
        let result = formatter.pretty();
        println!("{}", result);
    }
    if !error.is_empty() {
        println!("Error:    {}", error);
    }
    println!();
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
