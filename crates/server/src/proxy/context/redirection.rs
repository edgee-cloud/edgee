use url::Url;

use super::incoming::RequestHandle;
use crate::config;

pub struct RedirectionContext {
    pub destination: String,
}

impl RedirectionContext {
    pub fn from_request(request: &RequestHandle) -> Option<Self> {
        let cfg = &config::get().redirections;
        cfg.iter()
            .filter_map(|r| {
                Url::parse(&r.origin)
                    .ok()
                    .filter(|parsed_url| parsed_url.path() == request.get_path())
                    .map(|_| RedirectionContext {
                        destination: r.destination.clone(),
                    })
            })
            .next()
    }
}
