use tracing::info;

use crate::data_collection::payload::{Consent, EventType};

use super::debug::DebugParams;

pub struct Logger {}

impl Logger {
    pub fn log_outgoing_event(params: &DebugParams, duration: u128, error: &str) {
        let path = params.event.context.page.path.clone();
        let event_type = match params.event.event_type {
            EventType::Page => "page",
            EventType::Track => "track",
            EventType::User => "user",
        };
        let consent = match params.event.consent.clone().unwrap() {
            Consent::Granted => "granted",
            Consent::Denied => "denied",
            Consent::Pending => "pending",
        };
        info!(
            project = params.project_id,
            host = params.proxy_host,
            from = params.from,
            ip = params.client_ip,
            proxy_type = params.proxy_type,
            proxy_desc = params.proxy_desc,
            as_name = params.as_name,
            as_number = params.as_number,

            path = path,
            consent = consent,
            event = event_type,

            type = "dc:outgoing-event",
            component_id = params.component_id,
            component = params.component_slug,
            status = params.response_status,
            duration = duration,
            message = error
        );
    }
}
