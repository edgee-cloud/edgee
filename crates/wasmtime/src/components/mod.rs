use std::str::FromStr;
use std::time::Duration;

use config::ComponentsConfiguration;
use http::{header, HeaderMap, HeaderName, HeaderValue};
use json_pretty::PrettyFormatter;
use tracing::{error, info, span, Instrument, Level};

use context::ComponentsContext;

use crate::{
    exports::edgee::protocols::provider::{self},
    payload::{Consent, Event, EventType},
};
pub mod config;
pub mod config_file;
pub mod context;
mod convert;

pub async fn send_data_collection(
    ctx: &ComponentsContext,
    events: &mut [Event],
    component_config: &ComponentsConfiguration,
    log_component: &Option<String>,
) -> anyhow::Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let request_info = RequestInfo::new(events);

    let mut store = ctx.empty_store();

     // iterate on each event
   for mut event in events.iter_mut() {
        for cfg in component_config.get_collections().iter() {
            let span = span!(
                Level::INFO,
                "component",
                name = cfg.name.as_str(),
                event = event.event_type.to_string()
            );
            let _enter = span.enter();

            let debug =
            log_component.is_some() && log_component.as_ref().unwrap() == cfg.name.as_str();

            // if event_type is not enabled in ccfg.config, skip the event
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
                event.context.as_mut().unwrap().client.as_mut().unwrap().ip =
                    Some(request_info.ip_anonymized.clone());
                // todo: anonymize other data, utm, referrer, etc.
            } else {
                event.context.as_mut().unwrap().client.as_mut().unwrap().ip =
                    Some(request_info.ip.clone());
            }

            // Convert the event to the one which can be passed to the component
            let provider_event: provider::Event = event.clone().into();
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?;
            let instance = ctx
                .instantiate_data_collection(&cfg.name, &mut store)
                .await?;
            let provider = instance.edgee_protocols_provider();
            let credentials: Vec<(String, String)> = cfg.credentials.clone().into_iter().collect();

            let request = match provider_event.event_type {
                provider::EventType::Page => {
                    provider
                        .call_page(&mut store, &provider_event, &credentials)
                        .await
                }
                provider::EventType::Track => {
                    provider
                        .call_track(&mut store, &provider_event, &credentials)
                        .await
                }
                provider::EventType::User => {
                    provider
                        .call_user(&mut store, &provider_event, &credentials)
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
            insert_expected_headers(&mut headers, &event, &provider_event)?;

            let client = client.clone();

            let method_str = match request.method {
                provider::HttpMethod::Get => "GET",
                provider::HttpMethod::Put => "PUT",
                provider::HttpMethod::Post => "POST",
                provider::HttpMethod::Delete => "DELETE",
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
                        provider::HttpMethod::Get => {
                            client.get(request.url).headers(headers).send().await
                        }
                        provider::HttpMethod::Put => {
                            client
                                .put(request.url)
                                .headers(headers)
                                .body(request.body)
                                .send()
                                .await
                        }
                        provider::HttpMethod::Post => {
                            client
                                .post(request.url)
                                .headers(headers)
                                .body(request.body)
                                .send()
                                .await
                        }
                        provider::HttpMethod::Delete => {
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
}

impl RequestInfo {
    pub fn new(events: &[Event]) -> Self {
        let mut request_info = RequestInfo {
            from: "-".to_string(),
            ip: "".to_string(),
            ip_anonymized: "".to_string(),
            consent: "default".to_string(),
        };
        if let Some(event) = events.first() {
            // set request_info from the first event
            request_info.from = event.from.clone().unwrap_or("-".to_string());
            request_info.ip = event
                .context
                .as_ref()
                .unwrap()
                .client
                .as_ref()
                .unwrap()
                .ip
                .clone()
                .unwrap_or("".to_string());
            request_info.ip_anonymized = anonymize_ip(request_info.ip.clone());
            if event.consent.is_some() {
                request_info.consent = event.consent.as_ref().unwrap().to_string();
            }
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
    provider_event: &provider::Event,
) -> anyhow::Result<()> {
    // Insert client ip in the x-forwarded-for header
    headers.insert(
        HeaderName::from_str("x-forwarded-for")?,
        HeaderValue::from_str(&provider_event.context.client.ip)?,
    );

    // Insert User-Agent in the user-agent header
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_str(&provider_event.context.client.user_agent)?,
    );

    if let Some(context) = &event.context {
        if let Some(page) = &context.page {
            // Insert referrer in the referer header like an analytics client-side collect does
            if let Some(url) = &page.url {
                let document_location =
                    format!("{}{}", url, page.search.clone().unwrap_or_default());
                headers.insert(
                    header::REFERER,
                    HeaderValue::from_str(document_location.as_str())?,
                );
            }
        }

        if let Some(client) = &context.client {
            // Insert Accept-Language in the accept-language header
            if let Some(accept_language) = &client.accept_language {
                headers.insert(
                    header::ACCEPT_LANGUAGE,
                    HeaderValue::from_str(accept_language.as_str())?,
                );
            }
            // Insert sec-ch-ua headers
            if let Some(user_agent_version_list) = &client.user_agent_version_list {
                let ch_ua_value = format_ch_ua_header(user_agent_version_list);
                headers.insert(
                    HeaderName::from_str("sec-ch-ua")?,
                    HeaderValue::from_str(ch_ua_value.as_str())?,
                );
            }
            // Insert sec-ch-ua-mobile header
            if let Some(user_agent_mobile) = &client.user_agent_mobile {
                let mobile_value = format!("?{}", user_agent_mobile);
                headers.insert(
                    HeaderName::from_str("sec-ch-ua-mobile")?,
                    HeaderValue::from_str(mobile_value.as_str())?,
                );
            }
            // Insert sec-ch-ua-platform header
            if let Some(os_name) = &client.os_name {
                let platform_value = format!("\"{}\"", os_name);
                headers.insert(
                    HeaderName::from_str("sec-ch-ua-platform")?,
                    HeaderValue::from_str(platform_value.as_str())?,
                );
            }
        }
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
    request: &provider::EdgeeRequest,
    headers: &HeaderMap,
    _incoming_consent: String,
    outgoing_consent: String,
    anonymization: bool,
) {
    if !debug {
        return;
    }

    let method_str = match request.method {
        provider::HttpMethod::Get => "GET",
        provider::HttpMethod::Put => "PUT",
        provider::HttpMethod::Post => "POST",
        provider::HttpMethod::Delete => "DELETE",
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
