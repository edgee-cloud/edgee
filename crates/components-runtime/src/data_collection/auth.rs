use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use http::{header::HeaderName, header::HeaderValue, HeaderMap};
use std::str::FromStr;
use std::time::Duration;
use tracing::error;
use wasmtime::Store;

use crate::config::DataCollectionComponents;
use crate::data_collection::exports::edgee::components::data_collection::{
    Dict, Guest, HttpMethod,
};

pub async fn get_dynamodb_client() -> Client {
    if std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok() {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Client::new(&config)
    } else {
        let config = aws_config::ConfigLoader::default()
            .endpoint_url("http://localhost:9000")
            .behavior_version(aws_config::BehaviorVersion::latest())
            .load()
            .await;
        Client::new(&config)
    }
}

pub async fn get_token_from_cache(
    cfg: &DataCollectionComponents,
) -> Result<Option<String>, aws_sdk_dynamodb::Error> {
    let client = get_dynamodb_client().await;
    let key = format!("Auth#{}#{}", cfg.project_component_id, cfg.id);
    let item = client
        .get_item()
        .table_name("edgee_auth_token_cache")
        .key("auth_key", AttributeValue::S(key))
        .send()
        .await?
        .item;

    Ok(item.and_then(|item| item.get("token")?.as_s().ok().map(|s| s.to_string())))
}

pub async fn cache_token(
    cfg: &DataCollectionComponents,
    token: &str,
) -> Result<(), aws_sdk_dynamodb::Error> {
    let client = get_dynamodb_client().await;
    let key = format!("Auth#{}#{}", cfg.project_component_id, cfg.id);
    client
        .put_item()
        .table_name("edgee_auth_token_cache")
        .item("auth_key", AttributeValue::S(key))
        .item("token", AttributeValue::S(token.to_string()))
        .item(
            "expiration",
            AttributeValue::N((chrono::Utc::now().timestamp() + 3600).to_string()),
        )
        .send()
        .await?;
    Ok(())
}

pub async fn fetch_token_from_auth(
    component: &Guest,
    store: &mut Store<crate::context::HostState>,
    settings: &Dict,
) -> Option<(String, HeaderMap)> {
    let auth_request = match component.call_authenticate(store, settings).await {
        Ok(Ok(req)) => req,
        Ok(Err(err)) => {
            error!("auth error: {err}");
            return None;
        }
        Err(err) => {
            error!("auth error: {err}");
            return None;
        }
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;

    let mut headers = HeaderMap::new();
    for (key, value) in &auth_request.headers {
        headers.insert(
            HeaderName::from_str(key).ok()?,
            HeaderValue::from_str(value).ok()?,
        );
    }

    let res = match auth_request.method {
        HttpMethod::Get => {
            client
                .get(auth_request.url)
                .headers(headers.clone())
                .send()
                .await
        }
        HttpMethod::Put => {
            client
                .put(auth_request.url)
                .headers(headers.clone())
                .body(auth_request.body)
                .send()
                .await
        }
        HttpMethod::Post => {
            client
                .post(auth_request.url)
                .headers(headers.clone())
                .body(auth_request.body)
                .send()
                .await
        }
        HttpMethod::Delete => {
            client
                .delete(auth_request.url)
                .headers(headers.clone())
                .send()
                .await
        }
    };

    match res {
        Ok(res) => {
            let body = res.text().await.unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(token) = json
                    .get(&auth_request.token_response_name)
                    .and_then(|v| v.as_str())
                {
                    return Some((token.to_string(), headers));
                }
            }
            error!("Failed to parse auth response");
            None
        }
        Err(err) => {
            error!("Request failed: {err}");
            None
        }
    }
}
