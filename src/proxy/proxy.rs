use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use http::response::Parts;
use http::{header, HeaderName, HeaderValue, Method};
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::body::Incoming;
use tracing::{error, info};

use crate::config::config;
use crate::proxy::compute::compute;
use crate::proxy::context::incoming::IncomingContext;
use crate::proxy::context::proxy::ProxyContext;
use crate::proxy::context::routing::RoutingContext;
use crate::proxy::controller::controller;
use crate::tools::path;

const EDGEE_HEADER: HeaderName = HeaderName::from_static("x-edgee");
const EDGEE_FULL_DURATION_HEADER: HeaderName = HeaderName::from_static("x-edgee-full-duration");
const EDGEE_COMPUTE_DURATION_HEADER: HeaderName =
    HeaderName::from_static("x-edgee-compute-duration");
const EDGEE_PROXY_DURATION_HEADER: HeaderName = HeaderName::from_static("x-edgee-proxy-duration");

pub type Request = http::Request<Incoming>;
pub type ResponseBody = BoxBody<Bytes, Infallible>;
pub type Response = http::Response<ResponseBody>;

pub async fn handle_request(
    request: Request,
    remote_addr: SocketAddr,
    proto: &str,
) -> anyhow::Result<Response> {
    // timer
    let timer_start = std::time::Instant::now();

    // set incoming context
    let incoming_ctx = IncomingContext::new(request, remote_addr, proto);

    // set several variables
    let is_debug_mode = incoming_ctx.is_debug_mode;
    let content_type = incoming_ctx
        .header(header::CONTENT_TYPE)
        .unwrap_or_default();
    let incoming_method = incoming_ctx.method().clone();
    let incoming_host = incoming_ctx.host().clone();
    let incoming_path = incoming_ctx.path().clone();
    let incoming_headers = incoming_ctx.headers().clone();
    let incoming_proto = if incoming_ctx.is_https {
        "https"
    } else {
        "http"
    };
    let client_ip = incoming_ctx.client_ip.clone();

    // Check if the request is HTTPS and if we should force HTTPS
    if !incoming_ctx.is_https
        && config::get().http.is_some()
        && config::get().http.as_ref().unwrap().force_https
    {
        info!(
            "301 - {} {}{} - {}ms",
            incoming_method,
            incoming_host,
            incoming_path,
            timer_start.elapsed().as_millis()
        );
        return controller::redirect_to_https(incoming_host, incoming_path);
    }

    // SDK path
    if incoming_method == Method::GET
        && (incoming_path.path() == "/_edgee/sdk.js"
            || (incoming_path.path().starts_with("/_edgee/libs/edgee.")
                && incoming_path.path().ends_with(".js")))
    {
        info!(
            "200 - {} {}{} - {}ms",
            incoming_method,
            incoming_host,
            incoming_path,
            timer_start.elapsed().as_millis()
        );
        return controller::sdk(incoming_path.as_str());
    }

    // event path, method POST and content-type application/json
    if incoming_method == Method::POST
        && content_type == "application/json"
        && (incoming_path.path() == "/_edgee/event"
            || path::validate(incoming_host.as_str(), incoming_path.path()))
    {
        info!(
            "204 - {} {}{} - {}ms",
            incoming_method,
            incoming_host,
            incoming_path,
            timer_start.elapsed().as_millis()
        );
        return controller::edgee_client_event(
            incoming_ctx,
            &incoming_host,
            &incoming_path,
            &incoming_headers,
            &client_ip,
        )
        .await;
    }

    // event path for third party integration (Edgee installed like a third party, and use localstorage)
    if incoming_path.path() == "/_edgee/csevent" {
        if incoming_method == Method::OPTIONS {
            info!(
                "200 - {} {}{} - {}ms",
                incoming_method,
                incoming_host,
                incoming_path,
                timer_start.elapsed().as_millis()
            );
            return controller::options("POST, OPTIONS");
        }
        if incoming_method == Method::POST && content_type == "application/json" {
            info!(
                "200 - {} {}{} - {}ms",
                incoming_method,
                incoming_host,
                incoming_path,
                timer_start.elapsed().as_millis()
            );
            return controller::edgee_client_event_from_third_party_sdk(
                incoming_ctx,
                &incoming_path,
                &incoming_headers,
                &client_ip,
            )
            .await;
        }
    }

    // define the backend
    let routing_ctx = match RoutingContext::from_request_context(&incoming_ctx) {
        None => {
            error!("backend not found");
            info!(
                "502 - {} {}{} - {}ms",
                incoming_method,
                incoming_host,
                incoming_path,
                timer_start.elapsed().as_millis()
            );
            return controller::bad_gateway_error();
        }
        Some(r) => r,
    };

    // Amend proxy request with useful headers
    let proxy_ctx = ProxyContext::new(incoming_ctx, &routing_ctx);

    // send request and get response
    let res = proxy_ctx.forward_request().await;
    match res {
        Ok(upstream) => {
            let (mut response_parts, incoming) = upstream.into_parts();
            let response_body = incoming.collect().await?.to_bytes();
            info!(
                "{} - {} {}{} - {}ms",
                response_parts.status.as_str(),
                incoming_method,
                incoming_host,
                incoming_path,
                timer_start.elapsed().as_millis()
            );

            // Only proxy in some cases
            match do_only_proxy(&incoming_method, &response_body, &response_parts) {
                Ok(_) => {}
                Err(reason) => {
                    set_edgee_header(&mut response_parts, reason);
                    set_duration_headers(
                        &mut response_parts,
                        is_debug_mode,
                        timer_start.elapsed().as_millis(),
                        None,
                    );
                    return Ok(controller::build_response(response_parts, response_body));
                }
            }

            set_edgee_header(&mut response_parts, "compute");
            let proxy_duration = timer_start.elapsed().as_millis();

            let mut body_str = String::from_utf8_lossy(&response_body).into_owned();

            // interpret what's in the body
            match compute::html_handler(
                &body_str,
                &incoming_host,
                &incoming_path,
                &incoming_headers,
                incoming_proto,
                &client_ip,
                &mut response_parts,
            )
            .await
            {
                Ok(document) => {
                    let mut page_event_param = r#" data-client-side="true""#;
                    let event_path_param =
                        format!(r#" data-event-path="{}""#, path::generate(&incoming_host));

                    let mut debug_script = "".to_string();
                    if !document.data_collection_events.is_empty() {
                        if is_debug_mode {
                            debug_script = format!(
                                r#"<script>var _edgee_events = {}</script>"#,
                                document.data_collection_events
                            );
                        }
                        page_event_param = r#" data-client-side="false""#;
                    }

                    // if the context is empty, we need to add an empty context script tag
                    let mut empty_data_layer = "";
                    if document.data_layer.is_empty() {
                        empty_data_layer = r#"<script id="__EDGEE_DATA_LAYER__" type="application/json">{}</script>"#;
                    }

                    if !document.inlined_sdk.is_empty() {
                        let new_tag = format!(
                            r#"{}{}<script{}{}>{}</script>"#,
                            debug_script,
                            empty_data_layer,
                            page_event_param,
                            event_path_param,
                            document.inlined_sdk.as_str(),
                        );
                        body_str =
                            body_str.replace(document.sdk_full_tag.as_str(), new_tag.as_str());
                    } else {
                        let new_tag = format!(
                            r#"{}{}<script{}{} async src="{}"></script>"#,
                            debug_script,
                            empty_data_layer,
                            page_event_param,
                            event_path_param,
                            document.sdk_src.as_str()
                        );
                        body_str =
                            body_str.replace(document.sdk_full_tag.as_str(), new_tag.as_str());
                    }
                }
                Err(reason) => {
                    set_edgee_header(&mut response_parts, reason);
                }
            };

            let full_duration = timer_start.elapsed().as_millis();
            let compute_duration = full_duration - proxy_duration;
            set_duration_headers(
                &mut response_parts,
                is_debug_mode,
                full_duration,
                Some(compute_duration),
            );

            Ok(controller::build_response(
                response_parts,
                Bytes::from(body_str),
            ))
        }
        Err(err) => {
            error!("backend request failed: {}", err);
            info!(
                "502 - {} {}{} - {}ms",
                incoming_method,
                incoming_host,
                incoming_path,
                timer_start.elapsed().as_millis()
            );
            controller::bad_gateway_error()
        }
    }
}

/// Sets the duration headers for the response.
///
/// # Arguments
///
/// * `response_parts` - A mutable reference to the response parts.
/// * `is_debug_mode` - A boolean indicating whether debug mode is enabled.
/// * `full_duration` - The full duration of the request in milliseconds.
/// * `compute_duration` - An optional duration of the compute phase in milliseconds.
///
/// # Logic
///
/// If debug mode is enabled, the function inserts the full duration into the response headers.
/// If a compute duration is provided, it is inserted into the response headers.
/// Additionally, if debug mode is enabled, the function calculates the proxy duration and inserts it into the response headers.
fn set_duration_headers(
    response_parts: &mut Parts,
    is_debug_mode: bool,
    full_duration: u128,
    compute_duration: Option<u128>,
) {
    if is_debug_mode {
        response_parts.headers.insert(
            EDGEE_FULL_DURATION_HEADER,
            HeaderValue::from_str(format!("{}ms", full_duration).as_str()).unwrap(),
        );
    }
    if let Some(duration) = compute_duration {
        response_parts.headers.insert(
            EDGEE_COMPUTE_DURATION_HEADER,
            HeaderValue::from_str(format!("{}ms", duration).as_str()).unwrap(),
        );
        if is_debug_mode {
            let proxy_duration = full_duration - duration;
            response_parts.headers.insert(
                EDGEE_PROXY_DURATION_HEADER,
                HeaderValue::from_str(format!("{}ms", proxy_duration).as_str()).unwrap(),
            );
        }
    }
}

/// Sets the x-edgee header for the response to know the process.
///
/// # Arguments
///
/// * `response_parts` - A mutable reference to the response parts.
/// * `process` - A string slice representing the process to be set in the header.
///
/// # Logic
///
/// The function inserts the process information into the response headers.
pub fn set_edgee_header(response_parts: &mut Parts, process: &str) {
    response_parts
        .headers
        .insert(EDGEE_HEADER, HeaderValue::from_str(process).unwrap());
}

/// Determines whether to proxy the request based on various conditions.
///
/// # Arguments
///
/// * `method` - The HTTP method of the request.
/// * `response_body` - The body of the response.
/// * `response_parts` - The parts of the response.
///
/// # Returns
///
/// * `Result<bool, &'static str>` - Returns `Ok(false)` if the request should not be proxied,
///   otherwise returns an `Err` with a reason.
///
/// # Errors
///
/// This function returns an error if any of the following conditions are met:
/// - The `proxy_only` configuration is set to true.
/// - The request method is HEAD, OPTIONS, TRACE, or CONNECT.
/// - The response status is redirection (3xx).
/// - The response status is informational (1xx).
/// - The response does not have a content type.
/// - The response content type is not `text/html`.
/// - The response body is empty.
/// - The response content encoding is not supported.
/// - The response content length is greater than the maximum compressed body size.
fn do_only_proxy(
    method: &Method,
    response_body: &Bytes,
    response_parts: &Parts,
) -> Result<bool, &'static str> {
    let response_headers = response_parts.headers.clone();
    let content_type = response_headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok());

    // if conf.proxy_only is true
    if config::get().compute.proxy_only {
        Err("proxy-only(conf)")?;
    }

    // if the request method is HEAD, OPTIONS, TRACE or CONNECT
    if method == Method::HEAD
        || method == Method::OPTIONS
        || method == Method::TRACE
        || method == Method::CONNECT
    {
        Err("proxy-only(method)")?;
    }

    // if response is redirection
    if response_parts.status.is_redirection() {
        Err("proxy-only(3xx)")?;
    }

    // if response is informational
    if response_parts.status.is_informational() {
        Err("proxy-only(1xx)")?;
    }

    // if the response doesn't have a content type
    if content_type.is_none() {
        Err("proxy-only(no-content-type)")?;
    }

    // if the response content type is not text/html
    if content_type.is_some() && !content_type.unwrap().to_string().starts_with("text/html") {
        Err("proxy-only(non-html)")?;
    }

    // if the response doesn't have a body
    if response_body.is_empty() {
        Err("proxy-only(no-body)")?;
    }

    Ok(false)
}
