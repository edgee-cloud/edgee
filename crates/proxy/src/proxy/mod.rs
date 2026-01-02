use std::convert::Infallible;
use std::net::SocketAddr;
use std::str::FromStr;

use bytes::Bytes;
use context::incoming::RequestHandle;
use http::response::Parts;
use http::{header, HeaderName, HeaderValue, Method};
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;
use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use tracing::{error, info};

use crate::{config, get_components_ctx};
use context::{
    body::ProxyBody, incoming::IncomingContext, proxy::ProxyContext,
    redirection::RedirectionContext, routing::RoutingContext,
};

pub mod compute;
pub(crate) mod context;
mod controller;

const EDGEE_HEADER: &str = "x-edgee";
const EDGEE_FULL_DURATION_HEADER: &str = "x-edgee-full-duration";
const EDGEE_COMPUTE_DURATION_HEADER: &str = "x-edgee-compute-duration";
const EDGEE_PROXY_DURATION_HEADER: &str = "x-edgee-proxy-duration";

pub const DATA_COLLECTION_ENDPOINT: &str = "/_edgee/event";
pub const DATA_COLLECTION_ENDPOINT_FROM_THIRD_PARTY_SDK: &str = "/_edgee/csevent";

pub type Request = http::Request<Incoming>;
type Response = http::Response<BoxBody<Bytes, Infallible>>;

pub async fn handle_request(
    http_request: Request,
    remote_addr: SocketAddr,
    proto: &str,
) -> anyhow::Result<Response> {
    // timer
    let timer_start = std::time::Instant::now();

    // set incoming context
    let ctx = IncomingContext::new(http_request, remote_addr, proto);
    let request = &ctx.get_request().clone();

    // Check if the request is HTTPS and if we should force HTTPS
    if !request.is_https()
        && config::get().http.is_some()
        && config::get().http.as_ref().unwrap().force_https
    {
        info!(
            "301 - {} {}{} - {}ms",
            request.get_method(),
            request.get_host(),
            request.get_path(),
            timer_start.elapsed().as_millis()
        );
        return controller::redirect_to_https(request);
    }

    // SDK path
    if request.get_method() == Method::GET
        && (request.get_path() == "/_edgee/sdk.js"
            || (request.get_path().starts_with("/_edgee/libs/edgee.")
                && request.get_path().ends_with(".js")))
    {
        info!(
            "200 - {} {}{} - {}ms",
            request.get_method(),
            request.get_host(),
            request.get_path(),
            timer_start.elapsed().as_millis()
        );
        return controller::sdk(ctx);
    }

    // event by path
    if request.get_path() == DATA_COLLECTION_ENDPOINT {
        info!(
            "200 - {} {}{} - {}ms",
            request.get_method(),
            request.get_host(),
            request.get_path(),
            timer_start.elapsed().as_millis()
        );
        if request.get_method() == Method::OPTIONS {
            return controller::options(ctx, "POST, OPTIONS", true);
        }
        if is_request_post_json(request) {
            return controller::edgee_client_event(ctx, true).await;
        }
        return controller::empty_json_response();
    }

    // event like third party integration (Edgee installed like a third party, and use localstorage)
    if request.get_path() == DATA_COLLECTION_ENDPOINT_FROM_THIRD_PARTY_SDK {
        info!(
            "200 - {} {}{} - {}ms",
            request.get_method(),
            request.get_host(),
            request.get_path(),
            timer_start.elapsed().as_millis()
        );
        if request.get_method() == Method::OPTIONS {
            return controller::options(ctx, "POST, OPTIONS", false);
        }
        if is_request_post_json(request) {
            return controller::edgee_client_event_from_third_party_sdk(ctx).await;
        }
        return controller::empty_json_response();
    }

    // event by Authorization header
    if is_request_post_json(request) {
        if let Some(authorization) = request.get_header(header::AUTHORIZATION) {
            if edgee_dc_sdk::token::validate(request.get_host().as_str(), &authorization) {
                info!(
                    "200 - {} {}{} - {}ms",
                    request.get_method(),
                    request.get_host(),
                    request.get_path(),
                    timer_start.elapsed().as_millis()
                );
                return controller::edgee_client_event(ctx, false).await;
            }
        }
    }

    // redirection
    if let Some(redirection_ctx) = RedirectionContext::from_request(request) {
        info!(
            "302 - {} {}{} - {}ms",
            request.get_method(),
            request.get_host(),
            request.get_path(),
            timer_start.elapsed().as_millis()
        );
        return controller::build_redirection(&redirection_ctx);
    }

    // edge function
    for function in &config::get().components.edge_function {
        let invoke_path = function.settings.get("edgee_path");
        let invoke_path_prefix = function.settings.get("edgee_path_prefix");
        let active_methods = function.settings.get("edgee_function_active_methods");

        let match_path: bool = match (invoke_path, invoke_path_prefix) {
            (Some(path), None) => request.get_path() == path,
            (None, Some(prefix)) => request.get_path().starts_with(prefix),
            _ => false,
        };

        if match_path {
            let is_method_allowed = active_methods
                .is_none_or(|methods| methods.contains(request.get_method().as_str()));

            if is_method_allowed {
                let http_request = Request::from_parts(ctx.parts, ctx.body);
                let output = edgee_components_runtime::edge_function::invoke_fn(
                    get_components_ctx(),
                    &function.id,
                    &config::get().components,
                    http_request,
                )
                .await;
                return Ok(output.into());
            }
        }
    }

    // define the backend
    let routing_ctx = match RoutingContext::from_request(request) {
        None => {
            error!("backend not found");
            return controller::bad_gateway_error(request, timer_start);
        }
        Some(r) => r,
    };

    // Amend proxy request with useful headers
    let proxy_ctx = ProxyContext::new(ctx, &routing_ctx);

    // send request and get response
    let res = proxy_ctx.forward_request().await;
    match res {
        Err(err) => {
            error!("backend request failed: {}", err);
            controller::bad_gateway_error(request, timer_start)
        }
        Ok(upstream) => {
            let (mut response, body) = upstream.into_parts();
            info!(
                "{} - {} {}{} - {}ms",
                response.status.as_str(),
                request.get_method(),
                request.get_host(),
                request.get_path(),
                timer_start.elapsed().as_millis()
            );

            // Only proxy in some cases
            if let Some(reason) = do_only_proxy(request.get_method(), &body, &response) {
                set_edgee_header(&mut response, reason);
                set_duration_headers(
                    &mut response,
                    request.is_debug_mode(),
                    timer_start.elapsed().as_millis(),
                    None,
                );

                let response_body = body.collect_raw().await?;
                return Ok(controller::build_response(response, response_body));
            }

            let response_body = body.collect_all().await?;

            set_edgee_header(&mut response, "compute");
            let proxy_duration = timer_start.elapsed().as_millis();

            let mut body_str = String::from_utf8_lossy(&response_body).into_owned();

            // inject the sdk
            inject_sdk(&mut body_str, request.get_host());

            // interpret what's in the body
            match compute::html_handler(&body_str, request, &mut response).await {
                Ok(mut document) => {
                    let mut side_value = "c";
                    let mut debug_script = "".to_string();

                    if !document.data_collection_events.is_empty() {
                        if request.is_debug_mode() {
                            debug_script = format!(
                                r#"<script>var _edgee_events = {}</script>"#,
                                document.data_collection_events
                            );
                        }
                        side_value = "e";
                    }

                    // if the data_layer is empty, we need to add an empty data_layer script tag
                    let mut empty_data_layer = "";
                    if document.data_layer.is_empty() {
                        empty_data_layer = r#"<script id="__EDGEE_DATA_LAYER__" type="application/json">{}</script>"#;
                    }

                    if !document.inlined_sdk.is_empty() {
                        document.inlined_sdk = document.inlined_sdk.replace("{{side}}", side_value);
                        let new_tag = format!(
                            r#"{}{}<script>{}</script>"#,
                            debug_script,
                            empty_data_layer,
                            document.inlined_sdk.as_str(),
                        );
                        body_str =
                            body_str.replace(document.sdk_full_tag.as_str(), new_tag.as_str());
                    } else {
                        let new_tag = format!(
                            r#"{}{}<script async src="{}"></script>"#,
                            debug_script,
                            empty_data_layer,
                            document.sdk_src.as_str()
                        );
                        body_str =
                            body_str.replace(document.sdk_full_tag.as_str(), new_tag.as_str());
                    }
                }
                Err(reason) => {
                    set_edgee_header(&mut response, reason);
                }
            };

            let full_duration = timer_start.elapsed().as_millis();
            let compute_duration = full_duration - proxy_duration;
            set_duration_headers(
                &mut response,
                request.is_debug_mode(),
                full_duration,
                Some(compute_duration),
            );

            Ok(controller::build_response(response, Bytes::from(body_str)))
        }
    }
}

fn inject_sdk(body: &mut String, hostname: &str) {
    if !config::get().compute.inject_sdk {
        return;
    }
    let html_res = rewrite_str(
        body,
        RewriteStrSettings {
            element_content_handlers: vec![
                // first remove the existing sdk script if it exists
                element!("script#__EDGEE_SDK__", |el| {
                    el.remove();
                    Ok(())
                }),
                // add sdk to the head
                element!("head", |el| {
                    match config::get().compute.inject_sdk_position.as_str() {
                        "prepend" => {
                            el.prepend(
                                &format!(r#"<script id="__EDGEE_SDK__" async src="https://{hostname}/_edgee/sdk.js"></script>"#),
                                ContentType::Html,
                            );
                        }
                        _ => {
                            el.append(
                                &format!(r#"<script id="__EDGEE_SDK__" async src="https://{hostname}/_edgee/sdk.js"></script>"#),
                                ContentType::Html,
                            );
                        }
                    }
                    Ok(())
                }),
            ],
            ..RewriteStrSettings::new()
        },
    );

    if let Ok(html) = html_res {
        *body = html;
    }
}

fn is_request_post_json(request: &RequestHandle) -> bool {
    request.get_method() == Method::POST && request.get_content_type() == "application/json"
}

/// Sets the duration headers for the response.
///
/// # Arguments
///
/// * `response` - A mutable reference to the response parts.
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
    response: &mut Parts,
    is_debug_mode: bool,
    full_duration: u128,
    compute_duration: Option<u128>,
) {
    if is_debug_mode {
        response.headers.insert(
            HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
            HeaderValue::from_str(format!("{full_duration}ms").as_str()).unwrap(),
        );
    }
    if let Some(duration) = compute_duration {
        response.headers.insert(
            HeaderName::from_str(EDGEE_COMPUTE_DURATION_HEADER).unwrap(),
            HeaderValue::from_str(format!("{duration}ms").as_str()).unwrap(),
        );
        if is_debug_mode {
            let proxy_duration = full_duration - duration;
            response.headers.insert(
                HeaderName::from_str(EDGEE_PROXY_DURATION_HEADER).unwrap(),
                HeaderValue::from_str(format!("{proxy_duration}ms").as_str()).unwrap(),
            );
        }
    }
}

/// Sets the x-edgee header for the response to know the process.
///
/// # Arguments
///
/// * `response` - A mutable reference to the response parts.
/// * `process` - A string slice representing the process to be set in the header.
///
/// # Logic
///
/// The function inserts the process information into the response headers.
pub fn set_edgee_header(response: &mut Parts, process: &str) {
    response.headers.insert(
        HeaderName::from_str(EDGEE_HEADER).unwrap(),
        HeaderValue::from_str(process).unwrap(),
    );
}

/// Determines whether to proxy the request based on various conditions.
///
/// # Arguments
///
/// * `method` - The HTTP method of the request.
/// * `response_body` - The body of the response.
/// * `response` - The parts of the response.
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
    response_body: &ProxyBody,
    response: &Parts,
) -> Option<&'static str> {
    let response_headers = response.headers.clone();
    let content_type = response_headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok());

    // if conf.proxy_only is true
    if config::get().compute.proxy_only {
        return Some("proxy-only(conf)");
    }

    // if the request method is HEAD, OPTIONS, TRACE or CONNECT
    if method == Method::HEAD
        || method == Method::OPTIONS
        || method == Method::TRACE
        || method == Method::CONNECT
    {
        return Some("proxy-only(method)");
    }

    // if the response is a client error but 404
    if response.status.is_client_error() && response.status != http::StatusCode::NOT_FOUND {
        return Some("proxy-only(4xx)");
    }

    // if response is redirection
    if response.status.is_redirection() {
        return Some("proxy-only(3xx)");
    }

    // if response is informational
    if response.status.is_informational() {
        return Some("proxy-only(1xx)");
    }

    // if the response doesn't have a content type
    if content_type.is_none() {
        return Some("proxy-only(no-content-type)");
    }

    // if the response content type is not text/html
    if content_type.is_some() && !content_type.unwrap().to_string().starts_with("text/html") {
        return Some("proxy-only(non-html)");
    }

    // if the response doesn't have a body
    if response_body.is_empty() {
        return Some("proxy-only(no-body)");
    }

    None
}
