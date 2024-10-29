use std::convert::Infallible;
use std::net::SocketAddr;
use std::str::FromStr;

use bytes::Bytes;
use http::response::Parts;
use http::{header, HeaderName, HeaderValue, Method};
use http_body_util::{combinators::BoxBody, BodyExt, Either};
use hyper::body::{Body, Incoming};
use tracing::{error, info, warn};

use crate::config::config;
use crate::proxy::compute::compute;
use crate::proxy::context::incoming::IncomingContext;
use crate::proxy::context::proxy::ProxyContext;
use crate::proxy::context::routing::RoutingContext;
use crate::proxy::controller::controller;
use crate::tools::path;

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
        return controller::sdk(request.get_path().as_str());
    }

    // event path, method POST and content-type application/json
    if request.get_method() == Method::POST && request.get_content_type() == "application/json" {
        if request.get_path() == DATA_COLLECTION_ENDPOINT
            || path::validate(request.get_host().as_str(), request.get_path())
        {
            info!(
                "204 - {} {}{} - {}ms",
                request.get_method(),
                request.get_host(),
                request.get_path(),
                timer_start.elapsed().as_millis()
            );
            return controller::edgee_client_event(ctx).await;
        }
    }

    // event path for third party integration (Edgee installed like a third party, and use localstorage)
    if request.get_path() == DATA_COLLECTION_ENDPOINT_FROM_THIRD_PARTY_SDK {
        if request.get_method() == Method::OPTIONS {
            info!(
                "200 - {} {}{} - {}ms",
                request.get_method(),
                request.get_host(),
                request.get_path(),
                timer_start.elapsed().as_millis()
            );
            return controller::options("POST, OPTIONS");
        }
        if request.get_method() == Method::POST && request.get_content_type() == "application/json"
        {
            info!(
                "200 - {} {}{} - {}ms",
                request.get_method(),
                request.get_host(),
                request.get_path(),
                timer_start.elapsed().as_millis()
            );
            return controller::edgee_client_event_from_third_party_sdk(ctx).await;
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

            let response_body = match body {
                Either::Left(incoming) => incoming.collect().await?.to_bytes(),
                Either::Right(body) => {
                    let content_length = {
                        let data = body.get_ref();

                        data.size_hint().exact().unwrap() as usize
                    };

                    if content_length > config::get().compute.max_compressed_body_size {
                        warn!(
                            "compressed body too large: {content_length} > {}",
                            config::get().compute.max_compressed_body_size
                        );

                        let data = body.into_inner().collect().await?.to_bytes();

                        set_edgee_header(&mut response, "proxy-only(compressed-body-too-large)");
                        set_duration_headers(
                            &mut response,
                            request.is_debug_mode(),
                            timer_start.elapsed().as_millis(),
                            None,
                        );
                        return Ok(controller::build_response(response, data));
                    } else {
                        response.headers.remove("content-encoding");
                        response.headers.remove("content-length");

                        body.collect()
                            .await
                            .map_err(|err| anyhow::anyhow!(err))?
                            .to_bytes()
                    }
                }
            };

            // Only proxy in some cases
            match do_only_proxy(request.get_method(), &response_body, &response) {
                Ok(_) => {}
                Err(reason) => {
                    set_edgee_header(&mut response, reason);
                    set_duration_headers(
                        &mut response,
                        request.is_debug_mode(),
                        timer_start.elapsed().as_millis(),
                        None,
                    );
                    return Ok(controller::build_response(response, response_body));
                }
            }

            set_edgee_header(&mut response, "compute");
            let proxy_duration = timer_start.elapsed().as_millis();

            let mut body_str = String::from_utf8_lossy(&response_body).into_owned();

            // interpret what's in the body
            match compute::html_handler(&body_str, request, &mut response).await {
                Ok(document) => {
                    let mut client_side_param = r#" data-client-side="true""#;
                    let event_path_param = format!(
                        r#" data-event-path="{}""#,
                        path::generate(request.get_host().as_str())
                    );

                    let mut debug_script = "".to_string();
                    if !document.data_collection_events.is_empty() {
                        if request.is_debug_mode() {
                            debug_script = format!(
                                r#"<script>var _edgee_events = {}</script>"#,
                                document.data_collection_events
                            );
                        }
                        client_side_param = r#" data-client-side="false""#;
                    }

                    // if the data_layer is empty, we need to add an empty data_layer script tag
                    let mut empty_data_layer = "";
                    if document.data_layer.is_empty() {
                        empty_data_layer = r#"<script id="__EDGEE_DATA_LAYER__" type="application/json">{}</script>"#;
                    }

                    if !document.inlined_sdk.is_empty() {
                        let new_tag = format!(
                            r#"{}{}<script{}{}>{}</script>"#,
                            debug_script,
                            empty_data_layer,
                            client_side_param,
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
                            client_side_param,
                            event_path_param,
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
            HeaderValue::from_str(format!("{}ms", full_duration).as_str()).unwrap(),
        );
    }
    if let Some(duration) = compute_duration {
        response.headers.insert(
            HeaderName::from_str(EDGEE_COMPUTE_DURATION_HEADER).unwrap(),
            HeaderValue::from_str(format!("{}ms", duration).as_str()).unwrap(),
        );
        if is_debug_mode {
            let proxy_duration = full_duration - duration;
            response.headers.insert(
                HeaderName::from_str(EDGEE_PROXY_DURATION_HEADER).unwrap(),
                HeaderValue::from_str(format!("{}ms", proxy_duration).as_str()).unwrap(),
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
    response_body: &Bytes,
    response: &Parts,
) -> Result<bool, &'static str> {
    let response_headers = response.headers.clone();
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
    if response.status.is_redirection() {
        Err("proxy-only(3xx)")?;
    }

    // if response is informational
    if response.status.is_informational() {
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
