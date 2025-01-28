use super::incoming::RequestHandle;
use crate::config;

pub struct RedirectionContext {
    pub destination: String,
}

impl RedirectionContext {
    pub fn from_request(request: &RequestHandle) -> Option<Self> {
        let cfg = &config::get().redirections;
        cfg.iter()
            .find(|r| {
                r.origin.as_str()
                    == format!(
                        "{}://{}{}",
                        request.get_proto(),
                        request.get_host(),
                        request.get_path()
                    )
            })
            .map(|redirection| Self {
                destination: redirection.destination.clone(),
            })
    }
}
