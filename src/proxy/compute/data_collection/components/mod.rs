use std::str::FromStr;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue};
use tracing::{error, info, Instrument};

use crate::config::config;
use crate::proxy::compute::data_collection::payload::Payload;
use context::ComponentsContext;
use exports::provider;

mod context;
mod convert;

wasmtime::component::bindgen!({
    world: "data-collection",
    path: "wit/protocols.wit",
    async: true,
});

pub fn init() {
    ComponentsContext::init().unwrap();
}

pub async fn send_data_collection(p: Payload) -> anyhow::Result<()> {
    let config = config::get();

    // Fail early in case of invalid payload
    if p.event_type.is_none() {
        anyhow::bail!("invalid event type");
    }

    let ctx = ComponentsContext::get();
    let mut store = ctx.empty_store();

    // Convert the payload to the one which can be passed to the component
    let payload: provider::Payload = p.clone().into();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let event_str = match payload.event_type {
        provider::EventType::Page => "page",
        provider::EventType::Identify => "identify",
        provider::EventType::Track => "track",
    };

    let forwarded_for = HeaderValue::from_str(&payload.client.ip)?;
    let user_agent = HeaderValue::from_str(&payload.client.user_agent)?;

    for cfg in config.components.data_collection.iter() {
        if !p.is_destination_enabled(&cfg.name) {
            continue;
        }
        let instance = ctx
            .instantiate_data_collection(&cfg.name, &mut store)
            .await?;
        let provider = instance.provider();
        let credentials: Vec<(String, String)> = cfg.credentials.clone().into_iter().collect();

        let request = match payload.event_type {
            provider::EventType::Page => {
                provider.call_page(&mut store, &payload, &credentials).await
            }
            provider::EventType::Track => {
                provider
                    .call_track(&mut store, &payload, &credentials)
                    .await
            }
            provider::EventType::Identify => {
                provider
                    .call_identify(&mut store, &payload, &credentials)
                    .await
            }
        };
        let request = match request {
            Ok(Ok(request)) => request,
            Ok(Err(err)) => {
                error!(
                    provider = cfg.name,
                    event = event_str,
                    err = err.to_string(),
                    "failed to handle data collection payload"
                );
                continue;
            }
            Err(err) => {
                error!(
                    provider = cfg.name,
                    event = event_str,
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
        headers.insert(
            HeaderName::from_str("x-forwarded-for")?,
            forwarded_for.clone(),
        );
        headers.insert(http::header::USER_AGENT, user_agent.clone());

        let client = client.clone();

        // spawn a separated async thread
        tokio::spawn(
            async move {
                let method_str = match request.method {
                    provider::HttpMethod::Get => "GET",
                    provider::HttpMethod::Put => "PUT",
                    provider::HttpMethod::Post => "POST",
                    provider::HttpMethod::Delete => "DELETE",
                };
                info!(
                    step = "request",
                    provider = cfg.name,
                    event = event_str,
                    method = method_str,
                    url = request.url,
                    body = request.body
                );

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
                        if res.status().is_success() {
                            let status_str = format!("{:?}", res.status());
                            let body_res_str = res.text().await.unwrap_or_default();
                            info!(
                                step = "response",
                                provider = cfg.name,
                                event = event_str,
                                method = method_str,
                                status = status_str,
                                body = body_res_str
                            );
                        } else {
                            let status_str = format!("{:?}", res.status());
                            let body_res_str = res.text().await.unwrap_or_default();
                            error!(
                                step = "response",
                                provider = cfg.name,
                                event = event_str,
                                method = method_str,
                                status = status_str,
                                body = body_res_str
                            );
                        }
                    }
                    Err(err) => {
                        error!(
                            step = "response",
                            provider = cfg.name,
                            event = event_str,
                            method = method_str,
                            err = err.to_string()
                        );
                    }
                }
            }
            .in_current_span(),
        );
    }

    Ok(())
}
