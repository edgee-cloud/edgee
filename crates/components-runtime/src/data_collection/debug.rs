use crate::data_collection::payload::{Consent, Context, Data, Event, EventType};
use chrono::{DateTime, Utc};
use http::HeaderMap;
use json_pretty::PrettyFormatter;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

use super::{context::EventContext, logger::Logger};

#[derive(Serialize, Debug, Clone, Default)]
pub struct DebugPayload {
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub from: String,
    pub data: Data,
    pub context: Context,
    pub incoming_consent: String,
    pub outgoing_consent: String,
    pub anonymization: String,
    pub project_id: String,
    pub component_id: String,
    pub component_slug: String,
    pub component_request: DebugComponentRequest,
    pub component_response: DebugComponentResponse,
}

impl DebugPayload {
    pub fn new(params: DebugParams, error: &str) -> Self {
        let component_request = DebugComponentRequest::new(
            &params.method.to_string(),
            &params.url,
            &params.headers,
            &params.body,
        );

        let mut body = params.response_body;
        if !error.is_empty() {
            body = Some(error.to_string())
        }
        let component_response = DebugComponentResponse::new(
            params.response_status,
            body,
            params.response_content_type.to_string(),
            params.timer.elapsed().as_millis() as i32,
        );

        Self {
            uuid: params.event.uuid.clone(),
            timestamp: params.event.timestamp,
            event_type: match params.event.event_type {
                EventType::Page => "page".to_string(),
                EventType::User => "user".to_string(),
                EventType::Track => "track".to_string(),
            },
            from: params.from.to_string(),
            project_id: params.project_id.to_string(),
            data: params.event.data.clone(),
            context: params.event.context.clone(),
            incoming_consent: params.incoming_consent.to_string(),
            outgoing_consent: match params.event.consent.clone().unwrap() {
                Consent::Granted => "granted".to_string(),
                Consent::Denied => "denied".to_string(),
                Consent::Pending => "pending".to_string(),
            },
            anonymization: params.anonymization.to_string(),
            component_id: params.component_id.to_string(),
            component_slug: params.component_slug.to_string(),
            component_request,
            component_response,
        }
    }
}

#[derive(Clone)]
pub struct DebugParams {
    pub from: String,
    pub project_id: String,
    pub component_id: String,
    pub component_slug: String,
    pub event: Event,
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub response_content_type: String,
    pub response_status: i32,
    pub response_body: Option<String>,
    pub timer: std::time::Instant,
    pub anonymization: bool,
    pub incoming_consent: String,
    pub proxy_host: String,
    pub client_ip: String,
    pub proxy_type: String,
    pub proxy_desc: String,
    pub as_name: String,
    pub as_number: u32,
}

impl DebugParams {
    pub fn new(
        ctx: &EventContext,
        project_component_id: &str,
        component_slug: &str,
        event: &Event,
        method: &String,
        url: &String,
        headers: &HashMap<String, String>,
        body: &String,
        timer: std::time::Instant,
        anonymization: bool,
    ) -> DebugParams {
        DebugParams {
            from: ctx.get_from().clone(),
            project_id: ctx.get_project_id().clone(),
            component_id: project_component_id.to_string(),
            component_slug: component_slug.to_string(),
            event: event.clone(),
            method: method.to_string(),
            url: url.clone(),
            headers: headers.clone(),
            body: body.clone(),
            response_content_type: "".to_string(),
            response_status: 500,
            response_body: None,
            timer,
            anonymization,
            incoming_consent: ctx.get_consent().clone(),
            proxy_host: ctx.get_proxy_host().clone(),
            client_ip: ctx.get_ip().clone(),
            proxy_type: ctx.get_proxy_type().clone(),
            proxy_desc: ctx.get_proxy_desc().clone(),
            as_name: ctx.get_as_name().clone(),
            as_number: ctx.get_as_number(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DebugComponentRequest {
    pub method: String,
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<serde_json::Value>,
}

impl DebugComponentRequest {
    pub fn new(
        method: &String,
        url: &String,
        headers: &HashMap<String, String>,
        body: &String,
    ) -> DebugComponentRequest {
        let content_type = headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == "content-type")
            .map(|(_, v)| v.to_string())
            .unwrap_or_else(|| "text/plain".to_string());

        let b: Option<serde_json::Value> = if body.is_empty() {
            None
        } else if content_type.contains("application/json") || is_body_json(body.as_str()) {
            Some(
                serde_json::from_str(body.as_str())
                    .unwrap_or(serde_json::Value::String(body.clone())),
            )
        } else {
            Some(serde_json::Value::String(body.clone()))
        };

        DebugComponentRequest {
            method: method.to_string(),
            url: url.clone(),
            headers: Some(headers.clone()),
            body: b,
        }
    }
}

fn is_body_json(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body).is_ok()
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DebugComponentResponse {
    pub status_code: i32,
    pub body: Option<serde_json::Value>,
    pub content_type: String,
    pub duration: i32,
}

impl DebugComponentResponse {
    pub fn new(
        status_code: i32,
        body: Option<String>,
        content_type: String,
        duration: i32,
    ) -> DebugComponentResponse {
        let b: Option<serde_json::Value> = if body.is_none() {
            None
        } else if content_type.contains("application/json") {
            Some(
                serde_json::from_str(body.clone().unwrap().as_str())
                    .unwrap_or(serde_json::Value::String(body.clone().unwrap())),
            )
        } else {
            Some(serde_json::Value::String(body.clone().unwrap()))
        };

        DebugComponentResponse {
            status_code,
            body: b,
            content_type,
            duration,
        }
    }
}

pub fn trace_disabled_event(trace: bool, event: &str) {
    if !trace {
        return;
    }

    println!("--------------------------------------------");
    println!(" Event {} is disabled for this component", event);
    println!("--------------------------------------------\n");
}

pub fn trace_request(
    trace: bool,
    method: &String,
    url: &String,
    headers: &HeaderMap,
    body: &String,
    outgoing_consent: &String,
    anonymization: bool,
) {
    if !trace {
        return;
    }

    let anonymization_str = if anonymization { "true" } else { "false" };

    println!("-----------");
    println!("  REQUEST  ");
    println!("-----------\n");
    println!(
        "Config:   Consent: {}, Anonymization: {}",
        outgoing_consent, anonymization_str
    );
    println!("Method:   {}", method);
    println!("Url:      {}", url);
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

    if !body.is_empty() {
        println!("Body:");
        let formatter = PrettyFormatter::from_str(body.as_str());
        let result = formatter.pretty();
        println!("{}", result);
    } else {
        println!("Body:     None");
    }
    println!();
}

pub async fn debug_and_trace_response(
    debug: bool,
    trace: bool,
    params: DebugParams,
    error: String,
) -> anyhow::Result<()> {
    let elapsed = params.timer.elapsed();
    Logger::log_outgoing_event(&params, elapsed.as_millis(), &error);

    if trace {
        println!("------------");
        println!("  RESPONSE  ");
        println!("------------\n");
        println!("Status:   {}", params.response_status);
        println!("Duration: {}ms", elapsed.as_millis());
        if params.response_body.is_some() {
            if let Some(body) = params.response_body.clone() {
                println!("Body:");
                let formatter = PrettyFormatter::from_str(body.as_str());
                let result = formatter.pretty();
                println!("{}", result);
            }
        }
        if !error.is_empty() {
            println!("Error:    {}", &error);
        }
        println!();
    }

    if debug {
        let api_super_token = std::env::var("EDGEE_API_SUPER_TOKEN").unwrap_or_default();
        let api_url = std::env::var("EDGEE_API_URL").unwrap_or_default();

        if !api_super_token.is_empty() && !api_url.is_empty() && !params.project_id.is_empty() {
            let api_endpoint = format!("{}/v1/debug/data-collection", api_url);
            let debug_entry = DebugPayload::new(params, &error);
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?;
            let _r = client
                .post(api_endpoint.as_str())
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_super_token))
                .body(serde_json::to_string(&debug_entry).unwrap())
                .send()
                .await;
        }
    }

    Ok(())
}
