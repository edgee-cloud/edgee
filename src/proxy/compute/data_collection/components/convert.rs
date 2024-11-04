use std::collections::HashMap;

use super::exports::provider;
use crate::proxy::compute::data_collection::payload;

impl From<payload::Event> for provider::Event {
    fn from(value: payload::Event) -> Self {
        let data = match value.data {
            Some(payload::EventData::Page(page)) => provider::Data::Page(page.into()),
            Some(payload::EventData::User(user)) => provider::Data::User(user.into()),
            Some(payload::EventData::Track(track)) => provider::Data::Track(track.into()),
            None => todo!(),
        };
        Self {
            uuid: value.uuid,
            timestamp: value.timestamp.timestamp(),
            timestamp_millis: value.timestamp.timestamp_millis(),
            timestamp_micros: value.timestamp.timestamp_micros(),
            event_type: value.event_type.into(),
            data,
            context: value.context.unwrap_or_default().into(),
        }
    }
}

impl From<payload::EventType> for provider::EventType {
    fn from(value: payload::EventType) -> Self {
        match value {
            payload::EventType::Page => Self::Page,
            payload::EventType::User => Self::User,
            payload::EventType::Track => Self::Track,
        }
    }
}

impl From<payload::Page> for provider::PageData {
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
            properties: convert_dict(value.properties),
        }
    }
}

impl From<payload::User> for provider::UserData {
    fn from(value: payload::User) -> Self {
        Self {
            user_id: value.user_id.unwrap_or_default(),
            anonymous_id: value.anonymous_id.unwrap_or_default(),
            edgee_id: value.edgee_id,
            properties: convert_dict(value.properties),
        }
    }
}

impl From<payload::Track> for provider::TrackData {
    fn from(value: payload::Track) -> Self {
        Self {
            name: value.name.unwrap_or_default(),
            properties: convert_dict(value.properties),
        }
    }
}

impl From<payload::Context> for provider::Context {
    fn from(value: payload::Context) -> Self {
        Self {
            page: value.page.unwrap_or_default().into(),
            user: value.user.unwrap_or_default().into(),
            client: value.client.unwrap_or_default().into(),
            campaign: value.campaign.unwrap_or_default().into(),
            session: value.session.unwrap_or_default().into(),
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
            ip: value.ip.unwrap_or_default(),
            locale: value.locale.unwrap_or_default(),
            timezone: value.timezone.unwrap_or_default(),
            user_agent: value.user_agent.unwrap_or_default(),
            user_agent_architecture: value.user_agent_architecture.unwrap_or_default(),
            user_agent_bitness: value.user_agent_bitness.unwrap_or_default(),
            user_agent_full_version_list: value.user_agent_full_version_list.unwrap_or_default(),
            user_agent_version_list: value.user_agent_version_list.unwrap_or_default(),
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

fn convert_dict(dict: Option<HashMap<String, serde_json::Value>>) -> Vec<(String, String)> {
    let mut vec: Vec<(String, String)> = vec![];
    if dict.is_none() {
        return vec![];
    }
    for (k, v) in dict.unwrap().iter() {
        if v.is_object() || v.is_array() {
            continue;
        }
        let s = v.as_str();
        if let Some(s) = s {
            vec.push((k.clone(), s.to_string()));
        } else {
            vec.push((k.clone(), v.to_string()));
        }
    }
    vec
}
