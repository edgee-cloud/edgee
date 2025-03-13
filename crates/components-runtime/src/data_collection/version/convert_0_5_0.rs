use crate::data_collection::exports::edgee::components0_5_0::data_collection as DataCollection0_5_0;
use crate::data_collection::payload;
use crate::data_collection::version::convert::{convert_products, convert_properties};

impl From<payload::Event> for DataCollection0_5_0::Event {
    fn from(value: payload::Event) -> Self {
        let data = match value.data {
            payload::Data::Page(page) => DataCollection0_5_0::Data::Page(page.into()),
            payload::Data::User(user) => DataCollection0_5_0::Data::User(user.into()),
            payload::Data::Track(track) => DataCollection0_5_0::Data::Track(track.into()),
        };
        let consent = match value.consent {
            Some(payload::Consent::Pending) => Some(DataCollection0_5_0::Consent::Pending),
            Some(payload::Consent::Granted) => Some(DataCollection0_5_0::Consent::Granted),
            Some(payload::Consent::Denied) => Some(DataCollection0_5_0::Consent::Denied),
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

impl From<payload::EventType> for DataCollection0_5_0::EventType {
    fn from(value: payload::EventType) -> Self {
        match value {
            payload::EventType::Page => Self::Page,
            payload::EventType::User => Self::User,
            payload::EventType::Track => Self::Track,
        }
    }
}

impl From<payload::Page> for DataCollection0_5_0::PageData {
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

impl From<payload::User> for DataCollection0_5_0::UserData {
    fn from(value: payload::User) -> Self {
        Self {
            user_id: value.user_id,
            anonymous_id: value.anonymous_id,
            edgee_id: value.edgee_id,
            properties: convert_properties(value.properties.clone()),
        }
    }
}

impl From<payload::Track> for DataCollection0_5_0::TrackData {
    fn from(value: payload::Track) -> Self {
        Self {
            name: value.name,
            properties: convert_properties(value.properties.clone()),
            products: convert_products(value.properties.clone()),
        }
    }
}

impl From<payload::Context> for DataCollection0_5_0::Context {
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

impl From<payload::Campaign> for DataCollection0_5_0::Campaign {
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

impl From<payload::Client> for DataCollection0_5_0::Client {
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

impl From<payload::Session> for DataCollection0_5_0::Session {
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
