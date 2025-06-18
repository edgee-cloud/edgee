use crate::data_collection::convert::{convert_products, convert_properties};
use crate::data_collection::payload;
use crate::data_collection::versions::v1_0_1::data_collection::exports::edgee::components1_0_1::data_collection as DC;

impl From<payload::Event> for DC::Event {
    fn from(value: payload::Event) -> Self {
        let data = match value.data {
            payload::Data::Page(page) => DC::Data::Page(page.into()),
            payload::Data::User(user) => DC::Data::User(user.into()),
            payload::Data::Track(track) => DC::Data::Track(track.into()),
        };
        let consent = match value.consent {
            Some(payload::Consent::Pending) => Some(DC::Consent::Pending),
            Some(payload::Consent::Granted) => Some(DC::Consent::Granted),
            Some(payload::Consent::Denied) => Some(DC::Consent::Denied),
            None => None,
        };
        Self {
            uuid: value.uuid,
            timestamp: value.timestamp.timestamp(),
            timestamp_millis: value.timestamp.timestamp_millis(),
            timestamp_micros: value.timestamp.timestamp_micros(),
            event_type: value.event_type.into(),
            data,
            context: value.context.into(),
            consent,
        }
    }
}

impl From<payload::EventType> for DC::EventType {
    fn from(value: payload::EventType) -> Self {
        match value {
            payload::EventType::Page => Self::Page,
            payload::EventType::User => Self::User,
            payload::EventType::Track => Self::Track,
        }
    }
}

impl From<payload::Page> for DC::PageData {
    fn from(value: payload::Page) -> Self {
        Self {
            name: value.name,
            category: value.category,
            keywords: value.keywords,
            title: value.title,
            url: value.url,
            path: value.path,
            search: value.search,
            referrer: value.referrer,
            properties: convert_properties(value.properties.clone()),
        }
    }
}

impl From<payload::User> for DC::UserData {
    fn from(value: payload::User) -> Self {
        Self {
            user_id: value.user_id,
            anonymous_id: value.anonymous_id,
            edgee_id: value.edgee_id,
            properties: convert_properties(value.properties.clone()),
        }
    }
}

impl From<payload::Track> for DC::TrackData {
    fn from(value: payload::Track) -> Self {
        Self {
            name: value.name,
            properties: convert_properties(value.properties.clone()),
            products: convert_products(value.properties.clone()),
        }
    }
}

impl From<payload::Context> for DC::Context {
    fn from(value: payload::Context) -> Self {
        Self {
            page: value.page.into(),
            user: value.user.into(),
            client: value.client.into(),
            campaign: value.campaign.into(),
            session: value.session.into(),
        }
    }
}

impl From<payload::Campaign> for DC::Campaign {
    fn from(value: payload::Campaign) -> Self {
        Self {
            name: value.name,
            source: value.source,
            medium: value.medium,
            term: value.term,
            content: value.content,
            creative_format: value.creative_format,
            marketing_tactic: value.marketing_tactic,
        }
    }
}

impl From<payload::Client> for DC::Client {
    fn from(value: payload::Client) -> Self {
        Self {
            ip: value.ip,
            locale: value.locale,
            timezone: value.timezone,
            user_agent: value.user_agent,
            user_agent_architecture: value.user_agent_architecture,
            user_agent_bitness: value.user_agent_bitness,
            user_agent_full_version_list: value.user_agent_full_version_list,
            user_agent_version_list: value.user_agent_version_list,
            user_agent_mobile: value.user_agent_mobile,
            user_agent_model: value.user_agent_model,
            os_name: value.os_name,
            os_version: value.os_version,
            screen_width: value.screen_width,
            screen_height: value.screen_height,
            screen_density: value.screen_density,
            continent: value.continent,
            country_code: value.country_code,
            country_name: value.country_name,
            region: value.region,
            city: value.city,
        }
    }
}

impl From<payload::Session> for DC::Session {
    fn from(value: payload::Session) -> Self {
        Self {
            session_id: value.session_id,
            previous_session_id: value.previous_session_id,
            session_count: value.session_count,
            session_start: value.session_start,
            first_seen: value.first_seen.timestamp(),
            last_seen: value.last_seen.timestamp(),
        }
    }
}
