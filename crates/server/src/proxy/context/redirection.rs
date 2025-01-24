use tracing::info;

use super::incoming::RequestHandle;
use crate::config;

pub struct RedirectionContext {
    pub _origin: String,
    pub destination: String,
}

impl RedirectionContext {
    pub fn from_request(request: &RequestHandle) -> Option<Self> {
        let cfg = &config::get().redirections;
        let redirection = cfg
            .iter()
            .find(|r| {
                r.origin.as_str()
                    == format!(
                        "{}://{}{}",
                        request.get_proto(),
                        request.get_host(),
                        request.get_path()
                    )
            })?
            .to_owned();

        Some(Self {
            _origin: redirection.origin,
            destination: redirection.destination,
        })
    }
}
