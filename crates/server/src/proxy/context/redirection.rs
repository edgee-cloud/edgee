use super::incoming::RequestHandle;
use crate::config;

pub struct RedirectionContext {
    pub target: String,
}

impl RedirectionContext {
    pub fn from_request(request: &RequestHandle) -> Option<Self> {
        let cfg = &config::get().redirections;
        cfg.iter()
            .filter_map(|r| {
                if &r.source == request.get_path() {
                    Some(RedirectionContext {
                        target: r.target.clone(),
                    })
                } else {
                    None
                }
            })
            .next()
    }
}
