use std::collections::HashMap;

use super::exports::provider;
use crate::proxy::compute::data_collection::payload;

impl From<payload::Payload> for provider::Payload {
    fn from(value: payload::Payload) -> Self {
        Self {
            uuid: value.uuid,
            timestamp: value.timestamp.timestamp(),
            timestamp_millis: value.timestamp.timestamp_millis(),
            timestamp_micros: value.timestamp.timestamp_micros(),
            event_type: value.event_type.unwrap_or_default().into(),
            page: value.page.unwrap_or_default().into(),
            identify: value.identify.unwrap_or_default().into(),
            track: value.track.unwrap_or_default().into(),
            campaign: value.campaign.unwrap_or_default().into(),
            client: value.client.unwrap_or_default().into(),
            session: value.session.unwrap_or_default().into(),
        }
    }
}

impl From<payload::EventType> for provider::EventType {
    fn from(value: payload::EventType) -> Self {
        match value {
            payload::EventType::Page => Self::Page,
            payload::EventType::Identify => Self::Identify,
            payload::EventType::Track => Self::Track,
        }
    }
}

impl From<payload::Page> for provider::PageEvent {
    fn from(value: payload::Page) -> Self {
        Self {
            name: value.name.unwrap_or_default(),
            category: value.category.unwrap_or_default(),
            keywords: value.keywords.unwrap_or_default(),
            title: value.title.unwrap_or_default(),
            url: value.url.unwrap_or_default(),
            path: value.path.unwrap_or_default(),
            search: value.search.unwrap_or_default(),
            referrer: value.referrer.unwrap_or_default(),
            properties: convert_dict(value.properties.unwrap_or_default()),
        }
    }
}

impl From<payload::Identify> for provider::IdentifyEvent {
    fn from(value: payload::Identify) -> Self {
        Self {
            user_id: value.user_id.unwrap_or_default(),
            anonymous_id: value.anonymous_id.unwrap_or_default(),
            edgee_id: value.edgee_id,
            properties: convert_dict(value.properties.unwrap_or_default()),
        }
    }
}

impl From<payload::Track> for provider::TrackEvent {
    fn from(value: payload::Track) -> Self {
        Self {
            name: value.name.unwrap_or_default(),
            properties: convert_dict(value.properties.unwrap_or_default()),
        }
    }
}

impl From<payload::Campaign> for provider::Campaign {
    fn from(value: payload::Campaign) -> Self {
        Self {
            name: value.name.unwrap_or_default(),
            source: value.source.unwrap_or_default(),
            medium: value.medium.unwrap_or_default(),
            term: value.term.unwrap_or_default(),
            content: value.content.unwrap_or_default(),
            creative_format: value.creative_format.unwrap_or_default(),
            marketing_tactic: value.marketing_tactic.unwrap_or_default(),
        }
    }
}

impl From<payload::Client> for provider::Client {
    fn from(value: payload::Client) -> Self {
        Self {
            ip: anonymize_ip(&value.ip.unwrap_or_default()),
            locale: value.locale.unwrap_or_default(),
            timezone: value.timezone.unwrap_or_default(),
            user_agent: value.user_agent.unwrap_or_default(),
            user_agent_architecture: value.user_agent_architecture.unwrap_or_default(),
            user_agent_bitness: value.user_agent_bitness.unwrap_or_default(),
            user_agent_full_version_list: value.user_agent_full_version_list.unwrap_or_default(),
            user_agent_mobile: value.user_agent_mobile.unwrap_or_default(),
            user_agent_model: value.user_agent_model.unwrap_or_default(),
            os_name: value.os_name.unwrap_or_default(),
            os_version: value.os_version.unwrap_or_default(),
            screen_width: value.screen_width.unwrap_or_default(),
            screen_height: value.screen_height.unwrap_or_default(),
            screen_density: value.screen_density.unwrap_or_default(),
            continent: Default::default(),
            country_code: Default::default(),
            country_name: Default::default(),
            region: Default::default(),
            city: Default::default(),
        }
    }
}

impl From<payload::Session> for provider::Session {
    fn from(value: payload::Session) -> Self {
        Self {
            session_id: value.session_id,
            previous_session_id: value.previous_session_id.unwrap_or_default(),
            session_count: value.session_count,
            session_start: value.session_start,
            first_seen: value.first_seen.timestamp(),
            last_seen: value.last_seen.timestamp(),
        }
    }
}

fn convert_dict<T: ToString>(dict: HashMap<String, T>) -> Vec<(String, String)> {
    dict.into_iter()
        .map(|(key, value)| (key, value.to_string()))
        .collect()
}

fn anonymize_ip(ip: &str) -> String {
    use std::net::IpAddr;

    const KEEP_IPV4_BYTES: usize = 3;
    const KEEP_IPV6_BYTES: usize = 6;

    let ip: IpAddr = ip.parse().unwrap();
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
