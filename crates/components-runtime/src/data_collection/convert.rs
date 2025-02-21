use std::collections::HashMap;
use tracing::warn;

use crate::data_collection::exports::edgee::components::data_collection;
use crate::data_collection::payload;
impl From<payload::Event> for data_collection::Event {
    fn from(value: payload::Event) -> Self {
        let data = match value.data {
            payload::Data::Page(page) => data_collection::Data::Page(page.into()),
            payload::Data::User(user) => data_collection::Data::User(user.into()),
            payload::Data::Track(track) => data_collection::Data::Track(track.into()),
        };
        let consent = match value.consent {
            Some(payload::Consent::Pending) => Some(data_collection::Consent::Pending),
            Some(payload::Consent::Granted) => Some(data_collection::Consent::Granted),
            Some(payload::Consent::Denied) => Some(data_collection::Consent::Denied),
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

impl From<payload::EventType> for data_collection::EventType {
    fn from(value: payload::EventType) -> Self {
        match value {
            payload::EventType::Page => Self::Page,
            payload::EventType::User => Self::User,
            payload::EventType::Track => Self::Track,
        }
    }
}

impl From<payload::Page> for data_collection::PageData {
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

impl From<payload::User> for data_collection::UserData {
    fn from(value: payload::User) -> Self {
        Self {
            user_id: value.user_id,
            anonymous_id: value.anonymous_id,
            edgee_id: value.edgee_id,
            properties: convert_properties(value.properties.clone()),
        }
    }
}

impl From<payload::Track> for data_collection::TrackData {
    fn from(value: payload::Track) -> Self {
        Self {
            name: value.name,
            properties: convert_properties(value.properties.clone()),
            products: convert_products(value.properties.clone()),
        }
    }
}

impl From<payload::Context> for data_collection::Context {
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

impl From<payload::Campaign> for data_collection::Campaign {
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

impl From<payload::Client> for data_collection::Client {
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

impl From<payload::Session> for data_collection::Session {
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

fn convert_properties(properties: HashMap<String, serde_json::Value>) -> Vec<(String, String)> {
    use serde_json::Value;

    if properties.is_empty() {
        return Vec::new();
    };

    properties
        .into_iter()
        .filter(|(_, value)| !(value.is_array() || value.is_object()))
        .map(|(k, v)| {
            let value = if let Value::String(s) = v {
                s
            } else {
                v.to_string()
            };

            (k, value)
        })
        .collect()
}

fn convert_products(properties: HashMap<String, serde_json::Value>) -> Vec<Vec<(String, String)>> {
    use serde_json::Value;

    if properties.is_empty() {
        return Vec::new();
    };

    // if the key is products, then we need to convert the value to a list of tuples
    if let Some(products) = properties.get("products") {
        // if products is not an array, return an empty vector
        if !products.is_array() {
            warn!("data.properties.products is not an array, skipping");
            return Vec::new();
        }

        let mut results: Vec<Vec<(String, String)>> = Vec::new();
        let items = products.as_array().unwrap();
        items.iter().enumerate().for_each(|(index, product)| {
            // if product is not an object, go to the next product
            if !product.is_object() {
                warn!(
                    "data.properties.products[{}] is not an object, skipping",
                    index
                );
                return;
            }

            let mut i: Vec<(String, String)> = Vec::new();
            let dict = product.as_object().unwrap().clone();
            dict.into_iter()
                .filter(|(_, value)| !(value.is_array() || value.is_object()))
                .map(|(k, v)| {
                    let value = if let Value::String(s) = v {
                        s
                    } else {
                        v.to_string()
                    };
                    (k, value)
                })
                .for_each(|tuple| i.push(tuple));

            results.push(i);
        });
        results
    } else {
        Vec::new()
    }
}
