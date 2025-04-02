use chrono::{DateTime, Utc};

use crate::data_collection::payload::Event;

#[derive(Clone)]
pub struct EventContext {
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
    pub as_number: u32,
    pub project_id: String,
    pub proxy_host: String,
}

#[allow(dead_code)]
impl EventContext {
    pub fn new(events: &[Event], project_id: &str, proxy_host: &str) -> Self {
        let mut ctx = Self {
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
            as_number: 0,
            project_id: project_id.to_string(),
            proxy_host: proxy_host.to_string(),
        };
        if let Some(event) = events.first() {
            // set request_info from the first event
            ctx.from = event.from.clone().unwrap_or("-".to_string());
            ctx.ip = event.context.client.ip.clone();
            ctx.ip_anonymized = anonymize_ip(ctx.ip.clone());
            if event.consent.is_some() {
                ctx.consent = event.consent.as_ref().unwrap().to_string();
            }
            ctx.uuid = event.uuid.clone();
            ctx.timestamp = event.timestamp;
            ctx.edgee_id = event.context.user.edgee_id.clone();
            ctx.proxy_type = event
                .context
                .client
                .proxy_type
                .clone()
                .unwrap_or("-".to_string());
            ctx.proxy_desc = event
                .context
                .client
                .proxy_desc
                .clone()
                .unwrap_or("-".to_string());
            ctx.as_name = event
                .context
                .client
                .as_name
                .clone()
                .unwrap_or("-".to_string());
            ctx.as_number = event.context.client.as_number.unwrap_or(0);
        }
        ctx
    }

    pub fn get_from(&self) -> &String {
        &self.from
    }

    pub fn get_ip(&self) -> &String {
        &self.ip
    }

    pub fn get_ip_anonymized(&self) -> &String {
        &self.ip_anonymized
    }

    pub fn get_consent(&self) -> &String {
        &self.consent
    }

    pub fn get_uuid(&self) -> &String {
        &self.uuid
    }

    pub fn get_timestamp(&self) -> &DateTime<Utc> {
        &self.timestamp
    }

    pub fn get_edgee_id(&self) -> &String {
        &self.edgee_id
    }

    pub fn get_proxy_type(&self) -> &String {
        &self.proxy_type
    }

    pub fn get_proxy_desc(&self) -> &String {
        &self.proxy_desc
    }

    pub fn get_as_name(&self) -> &String {
        &self.as_name
    }

    pub fn get_as_number(&self) -> u32 {
        self.as_number
    }

    pub fn get_project_id(&self) -> &String {
        &self.project_id
    }

    pub fn get_proxy_host(&self) -> &String {
        &self.proxy_host
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
