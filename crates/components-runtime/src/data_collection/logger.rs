use http::StatusCode;
use tracing::info;

use super::RequestInfo;

pub(crate) struct Logger {}

impl Logger {
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn log_outgoing_event(
        request: &RequestInfo,
        component_id: &str,
        component_name: &str,
        status: StatusCode,
        event_type: &str,
        path: &str,
        consent: &str,
        duration: u128,
        message: &str,
    ) {
        info!(
            project = request.project_id,
            host = request.proxy_host,
            from = request.from,
            ip = request.ip,
            proxy_type = request.proxy_type,
            proxy_desc = request.proxy_desc,
            as_name = request.as_name,

            type = "dc:outgoing-event",
            component_id = component_id,
            component_name = component_name,
            status = status.to_string(),
            event_type = event_type,
            path = path,
            consent = consent,
            duration = duration,
            message
        );
        // todo: send a log to bigquery
    }
}

#[macro_export]
macro_rules! log_outgoing_event {
    // log_outgoing_event!(request, "component_id", "component_name", "200", "event_type", "path", "consent", 100, "message");
    ($request:expr, $component_id:expr, $component_name:expr, $status:expr, $event_type:expr, $path:expr, $consent:expr, $duration:expr, $message:expr) => {{
        $crate::data_collection::logger::Logger::log_outgoing_event(
            $request,
            $component_id,
            $component_name,
            $status,
            $event_type,
            $path,
            $consent,
            $duration,
            $message,
        );
    }};
}
