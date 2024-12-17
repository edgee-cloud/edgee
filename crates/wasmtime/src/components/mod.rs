use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use http::{header, HeaderMap, HeaderName, HeaderValue};
use json_pretty::PrettyFormatter;
use serde::Deserialize;
use tracing::{error, info, span, Instrument, Level};

use context::ComponentsContext;

use crate::{exports::provider, payload::Event};

pub mod context;
mod convert;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ComponentsConfiguration {
    pub data_collection: Vec<DataCollectionConfiguration>,
    pub cache: Option<PathBuf>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct DataCollectionConfiguration {
    pub name: String,
    pub component: String,
    pub credentials: HashMap<String, String>,
}

pub async fn send_data_collection(
    ctx: &ComponentsContext,
    events: &Vec<Event>,
    component_config: &ComponentsConfiguration,
    log_component: &Option<String>,
) -> anyhow::Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let mut store = ctx.empty_store();

    for event in events {
        // Convert the event to the one which can be passed to the component
        let mut provider_event: provider::Event = event.clone().into();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let event_str = match provider_event.event_type {
            provider::EventType::Page => "page",
            provider::EventType::User => "user",
            provider::EventType::Track => "track",
        };

        // todo: anonymize ip following the consent mapping
        provider_event.context.client.ip = anonymize_ip(provider_event.context.client.ip.clone());

        let client_ip = HeaderValue::from_str(&provider_event.context.client.ip)?;
        let user_agent = HeaderValue::from_str(&provider_event.context.client.user_agent)?;

        for cfg in component_config.data_collection.iter() {
            let span = span!(
                Level::INFO,
                "component",
                name = cfg.name.as_str(),
                event = event_str
            );
            let _enter = span.enter();

            if !event.is_component_enabled(&cfg.name) {
                continue;
            }
            let instance = ctx
                .instantiate_data_collection(&cfg.name, &mut store)
                .await?;
            let provider = instance.provider();
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
            insert_expected_headers(&mut headers, event, &client_ip, &user_agent)?;

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
            let log =
                log_component.is_some() && log_component.as_ref().unwrap() == cfg.name.as_str();

            if log {
                debug_request(&request, &headers);
            }

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
                            if log {
                                debug_response(
                                    &status_str,
                                    timer_start,
                                    body_res_str,
                                    "".to_string(),
                                );
                            }
                        }
                        Err(err) => {
                            error!(step = "response", status = "500", err = err.to_string());
                            if log {
                                debug_response("502", timer_start, "".to_string(), err.to_string());
                            }
                        }
                    }
                }
                .in_current_span(),
            );
        }
    }
    Ok(())
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
    client_ip: &HeaderValue,
    user_agent: &HeaderValue,
) -> anyhow::Result<()> {
    // Insert client ip in the x-forwarded-for header
    headers.insert(HeaderName::from_str("x-forwarded-for")?, client_ip.clone());

    // Insert User-Agent in the user-agent header
    headers.insert(header::USER_AGENT, user_agent.clone());

    // Insert referrer in the referer header like an analytics client-side collect does
    if event
        .context
        .as_ref()
        .unwrap()
        .page
        .as_ref()
        .unwrap()
        .url
        .is_some()
    {
        let document_location = format!(
            "{}{}",
            event
                .context
                .as_ref()
                .unwrap()
                .page
                .as_ref()
                .unwrap()
                .url
                .as_ref()
                .unwrap(),
            event
                .context
                .as_ref()
                .unwrap()
                .page
                .as_ref()
                .unwrap()
                .search
                .clone()
                .unwrap_or_default(),
        );
        headers.insert(
            header::REFERER,
            HeaderValue::from_str(document_location.as_str())?,
        );
    }

    // Insert Accept-Language in the accept-language header
    if event
        .context
        .as_ref()
        .unwrap()
        .client
        .as_ref()
        .unwrap()
        .accept_language
        .is_some()
    {
        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_str(
                event
                    .context
                    .as_ref()
                    .unwrap()
                    .client
                    .as_ref()
                    .unwrap()
                    .accept_language
                    .as_ref()
                    .unwrap(),
            )?,
        );
    }

    Ok(())
}

fn debug_request(request: &provider::EdgeeRequest, headers: &HeaderMap) {
    let method_str = match request.method {
        provider::HttpMethod::Get => "GET",
        provider::HttpMethod::Put => "PUT",
        provider::HttpMethod::Post => "POST",
        provider::HttpMethod::Delete => "DELETE",
    };

    println!("-----------");
    println!("  REQUEST  ");
    println!("-----------\n");
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

fn debug_response(status: &str, timer_start: std::time::Instant, body: String, error: String) {
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
