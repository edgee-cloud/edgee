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
                if &r.origin == request.get_path() {
                    Some(RedirectionContext {
                        destination: r.destination.clone(),
                    })
                } else {
                    None
                }
            })
            .next()
    }
}
