use crate::config::config;
use crate::proxy::compute::compute;
use crate::proxy::context::incoming::IncomingContext;
use crate::proxy::context::proxy::ProxyContext;
use crate::proxy::context::routing::RoutingContext;
use crate::proxy::controller::controller;
use crate::tools::path;
use brotli::{CompressorWriter, Decompressor};
use bytes::Bytes;
use http::response::Parts;
use http::{header, HeaderName, HeaderValue, Method};
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::body::Incoming;
use libflate::{deflate, gzip};
use std::{
    convert::Infallible,
    io::{Read, Write},
    net::SocketAddr,
    str::FromStr,
};
use tracing::{error, info, warn};

const EDGEE_HEADER: &str = "x-edgee";
const EDGEE_FULL_DURATION_HEADER: &str = "x-edgee-full-duration";
const EDGEE_COMPUTE_DURATION_HEADER: &str = "x-edgee-compute-duration";
const EDGEE_PROXY_DURATION_HEADER: &str = "x-edgee-proxy-duration";

type Response = http::Response<BoxBody<Bytes, Infallible>>;

pub async fn handle_request(
    http_request: http::Request<Incoming>,
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
        if request.get_path() == "/_edgee/event"
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
    if request.get_path() == "/_edgee/csevent" {
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
    let res = proxy_ctx.response().await;
    match res {
        Err(err) => {
            error!("backend request failed: {}", err);
            controller::bad_gateway_error(request, timer_start)
        }
        Ok(upstream) => {
            let (mut response, incoming) = upstream.into_parts();
            let response_body = incoming.collect().await?.to_bytes();
            info!(
                "{} - {} {}{} - {}ms",
                response.status.as_str(),
                request.get_method(),
                request.get_host(),
                request.get_path(),
                timer_start.elapsed().as_millis()
            );

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
            let response_headers = response.headers.clone();
            let encoding = response_headers
                .get(header::CONTENT_ENCODING)
                .and_then(|h| h.to_str().ok());

            // decompress the response body
            let cursor = std::io::Cursor::new(response_body.clone());
            let mut body_str = match encoding {
                Some("gzip") => {
                    let mut decoder = gzip::Decoder::new(cursor)?;
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf)?;
                    String::from_utf8_lossy(&buf).to_string()
                }
                Some("deflate") => {
                    let mut decoder = deflate::Decoder::new(cursor);
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf)?;
                    String::from_utf8_lossy(&buf).to_string()
                }
                Some("br") => {
                    let mut decoder = Decompressor::new(cursor, 4096);
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf)?;
                    String::from_utf8_lossy(&buf).to_string()
                }
                Some(_) | None => String::from_utf8_lossy(&response_body).to_string(),
            };

            // interpret what's in the body
            let _ = match compute::html_handler(&mut body_str, request, &mut response).await {
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

            let data = match encoding {
                Some("gzip") => {
                    let mut encoder = gzip::Encoder::new(Vec::new())?;
                    encoder.write_all(body_str.as_bytes())?;
                    encoder.finish().into_result()?
                }
                Some("deflate") => {
                    let mut encoder = deflate::Encoder::new(Vec::new());
                    encoder.write_all(body_str.as_bytes())?;
                    encoder.finish().into_result()?
                }
                Some("br") => {
                    // handle brotli encoding
                    // q: quality (range: 0-11), lgwin: window size (range: 10-24)
                    let mut encoder = CompressorWriter::new(Vec::new(), 4096, 11, 24);
                    encoder.write_all(body_str.as_bytes())?;
                    encoder.flush()?;
                    encoder.into_inner()
                }
                Some(_) | None => body_str.into(),
            };

            let full_duration = timer_start.elapsed().as_millis();
            let compute_duration = full_duration - proxy_duration;
            set_duration_headers(
                &mut response,
                request.is_debug_mode(),
                full_duration,
                Some(compute_duration),
            );

            Ok(controller::build_response(
                response,
                Bytes::from(data),
            ))
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
    let encoding = response_headers
        .get(header::CONTENT_ENCODING)
        .and_then(|h| h.to_str().ok());
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

    // if content-encoding is not supported
    if !["gzip", "deflate", "identity", "br", ""].contains(&encoding.unwrap_or_default()) {
        warn!("encoding not supported: {}", encoding.unwrap_or_default());
        Err("proxy-only(encoding-not-supported)")?;
    }

    // if the response is compressed and if content length is greater than the max_compressed_body_size configuration
    if ["gzip", "deflate", "br"].contains(&encoding.unwrap_or_default()) {
        if response_body.len() > config::get().compute.max_compressed_body_size {
            warn!(
                "compressed body too large: {} > {}",
                response_body.len(),
                config::get().compute.max_compressed_body_size
            );
            Err("proxy-only(compressed-body-too-large)")?;
        }
    }

    Ok(false)
}
