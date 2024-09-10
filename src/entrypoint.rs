use std::{
    collections::HashMap,
    convert::Infallible,
    io::{Read, Write},
    net::SocketAddr,
    str::FromStr,
};
use bytes::{Buf, Bytes};
use http::{
    header::{
        ACCEPT_LANGUAGE, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE,
        REFERER, USER_AGENT, LOCATION
    },
    HeaderMap, HeaderName, HeaderValue, Method, StatusCode,
};
use http::response::Parts;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::body::Incoming;
use incoming_context::IncomingContext;
use libflate::{deflate, gzip};
use proxy_context::ProxyContext;
use routing_context::RoutingContext;
use tracing::{debug, error, warn};
use crate::{tools::{edgee_cookie, path}, data_collection::{self, Session}, destinations, config};
use crate::tools::real_ip::Realip;
use crate::{data_collection::Payload, html};
use brotli::{Decompressor, CompressorWriter};

mod incoming_context;
mod proxy_context;
mod routing_context;
mod web;
mod websecure;

const EDGEE_HEADER: &str = "x-edgee";
const EDGEE_FULL_DURATION_HEADER: &str = "x-edgee-full-duration";
const EDGEE_COMPUTE_DURATION_HEADER: &str = "x-edgee-compute-duration";
const EDGEE_PROXY_DURATION_HEADER: &str = "x-edgee-proxy-duration";

type Response = http::Response<BoxBody<Bytes, Infallible>>;

pub async fn start() -> anyhow::Result<()> {
    let config =  config::get();
    let mut tasks = Vec::new();

    if config.http.is_some() {
        tasks.push(tokio::spawn(async {
            if let Err(err) = web::start().await {
                error!(?err, "Failed to start HTTP entrypoint");
            }
        }));
    }

    if config.https.is_some() {
        tasks.push(tokio::spawn(async {
            if let Err(err) = websecure::start().await {
                error!(?err, "Failed to start HTTPS entrypoint");
            }
        }));
    }

    tokio::select! {
        _ = tasks.pop().unwrap() => Ok(()),
    }
}


async fn handle_request(request: http::Request<Incoming>, remote_addr: SocketAddr, proto: &str) -> anyhow::Result<Response> {

    // timer
    let timer_start = std::time::Instant::now();

    // set incoming context
    let incoming_ctx = IncomingContext::new(request, remote_addr, proto);

    // set several variables
    let is_debug_mode = incoming_ctx.is_debug_mode;
    let content_type = incoming_ctx.header(CONTENT_TYPE).unwrap_or(String::new());
    let incoming_method = incoming_ctx.method().clone();
    let incoming_host = incoming_ctx.host().clone();
    let incoming_path = incoming_ctx.path().clone();
    let incoming_headers = incoming_ctx.headers().clone();

    // Check if the request is HTTPS and if we should force HTTPS
    if !incoming_ctx.is_https && config::get().http.is_some() && config::get().http.as_ref().unwrap().force_https {
        return Ok(http::Response::builder()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header(LOCATION, format!("https://{}{}", incoming_host, incoming_path))
            .header(CONTENT_TYPE, "text/plain")
            .body(empty())
            .expect("response builder should never fail"));
    }

    // SDK path
    if incoming_method == Method::GET && (incoming_path == "/_edgee/sdk.js" || (incoming_path.path().starts_with("/_edgee/libs/edgee.") && incoming_path.path().ends_with(".js"))) {
        debug!(?incoming_path, "serving sdk");
        return serve_sdk(incoming_path.as_str());
    }

    // event path, method POST and content-type application/json
    if incoming_method == Method::POST && content_type == "application/json" {
        if incoming_path == "/_edgee/event" || path::validate(incoming_host.as_str(), incoming_path.as_str()) {
            let mut res = http::Response::builder()
                .status(StatusCode::NO_CONTENT)
                .header(CACHE_CONTROL, "private, no-store")
                .body(empty())
                .unwrap();

            let cookie = edgee_cookie::get_or_set(
                &incoming_headers,
                res.headers_mut(),
                &incoming_host
            );

            let body = incoming_ctx.incoming_body.collect().await?.to_bytes();
            let result: Result<Payload, _> = serde_json::from_reader(body.reader());
            return match result {
                Ok(mut payload) => {
                    payload.uuid = uuid::Uuid::new_v4().to_string();
                    payload.timestamp = chrono::Utc::now();

                    let user_id = cookie.id.to_string();
                    payload.identify.edgee_id = user_id.clone();
                    payload.session = Session {
                        session_id: cookie.ss.timestamp().to_string(),
                        previous_session_id: cookie
                            .ps
                            .map(|t| t.timestamp().to_string())
                            .unwrap_or_default(),
                        session_count: cookie.sc,
                        session_start: cookie.ss == cookie.ls,
                        first_seen: cookie.fs,
                        last_seen: cookie.ls,
                    };

                    if payload.page.referrer.is_empty() {
                        let referrer = incoming_headers
                            .get(REFERER)
                            .and_then(|h| h.to_str().ok())
                            .map(String::from)
                            .unwrap_or_default();
                        payload.page.referrer = referrer;
                    }

                    payload.client.user_agent = incoming_headers
                        .get(USER_AGENT)
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    payload.client.x_forwarded_for = incoming_headers
                        .get("x-forwarded-for")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    payload.client.user_agent_architecture = incoming_headers
                        .get("sec-ch-ua-arch")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    payload.client.user_agent_bitness = incoming_headers
                        .get("sec-ch-ua-bitness")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    payload.client.user_agent_full_version_list = incoming_headers
                        .get("sec-ch-ua")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    payload.client.user_agent_mobile = incoming_headers
                        .get("sec-ch-ua-mobile")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    payload.client.user_agent_model = incoming_headers
                        .get("sec-ch-ua-model")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    payload.client.os_name = incoming_headers
                        .get("sec-ch-ua-platform")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    payload.client.os_version = incoming_headers
                        .get("sec-ch-ua-platform-version")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from)
                        .unwrap_or_default();

                    // client ip
                    let realip = Realip::new();
                    payload.client.ip = realip.get_from_request(remote_addr, &incoming_headers);

                    payload.client.locale = preferred_language(&incoming_headers);

                    let map: HashMap<String, String> =
                        url::form_urlencoded::parse(incoming_path.query().unwrap_or("").as_bytes())
                            .into_owned()
                            .collect();

                    payload.campaign.name = map
                        .get("utm_campaign")
                        .map(String::from)
                        .unwrap_or_default();

                    payload.campaign.source =
                        map.get("utm_source").map(String::from).unwrap_or_default();

                    payload.campaign.medium =
                        map.get("utm_medium").map(String::from).unwrap_or_default();

                    payload.campaign.term = map.get("utm_term").map(String::from).unwrap_or_default();

                    payload.campaign.content =
                        map.get("utm_content").map(String::from).unwrap_or_default();

                    payload.campaign.creative_format = map
                        .get("utm_creative_format")
                        .map(String::from)
                        .unwrap_or_default();

                    payload.campaign.marketing_tactic = map
                        .get("utm_marketing_tactic")
                        .map(String::from)
                        .unwrap_or_default();

                    if let Err(err) = destinations::send_data_collection(payload).await {
                        warn!(?err, "failed to process data collection");
                    }

                    Ok(res)
                }
                Err(err) => {
                    warn!(?err, "failed to parse json payload");
                    Ok(res)
                }
            };
        }
    }

    // define the backend
    let routing_ctx = match RoutingContext::from_request_context(&incoming_ctx) {
        None => return Ok(error_bad_gateway()),
        Some(r) => r,
    };

    // Amend proxy request with useful headers
    let proxy_ctx = ProxyContext::new(incoming_ctx, &routing_ctx);

    // send request and get response
    let res = proxy_ctx.response().await;
    match res {
        Err(err) => {
            error!(?err, "backend request failed");
            Ok(error_bad_gateway())
        }
        Ok(upstream) => {
            let (mut response_parts, incoming) = upstream.into_parts();
            let response_body = incoming.collect().await?.to_bytes();
            let response_headers = response_parts.headers.clone();
            let encoding = response_headers.get(CONTENT_ENCODING).and_then(|h| h.to_str().ok());
            let content_type = response_headers.get(CONTENT_TYPE).and_then(|h| h.to_str().ok());
            let content_length = response_headers.get(CONTENT_LENGTH).and_then(|h| h.to_str().ok());

            debug!(
                ?encoding,
                ?content_type,
                ?content_length,
                "upstream response"
            );

            //** START: Only proxy in some cases

            // if the request method is HEAD, OPTIONS, TRACE or CONNECT, just return the response
            if incoming_method == Method::HEAD || incoming_method == Method::OPTIONS || incoming_method == Method::TRACE || incoming_method == Method::CONNECT {
                set_edgee_header(&mut response_parts, "proxy-only(method)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }

            // if response is informational or redirection, just return the response
            if response_parts.status.is_redirection() {
                set_edgee_header(&mut response_parts, "proxy-only(3xx)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }
            if response_parts.status.is_informational() {
                set_edgee_header(&mut response_parts, "proxy-only(1xx)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }

            // if the response content type is not text/html, just return the response
            if !content_type.unwrap().starts_with("text/html") {
                set_edgee_header(&mut response_parts, "proxy-only(non-html)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }
            if content_type.is_none() {
                set_edgee_header(&mut response_parts, "proxy-only(no-content-type)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }

            // if the response hasn't a body, return true
            if response_body.is_empty() {
                set_edgee_header(&mut response_parts, "proxy-only(no-body)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }

            // if the response content length is greater than the max compressed body size, return true
            if content_length.is_some() && routing_ctx.rule.max_compressed_body_size.is_some() {
                let content_length = content_length.and_then(|s| s.parse::<u64>().ok()).unwrap();
                let size_limit = routing_ctx.rule.max_compressed_body_size.unwrap();
                if content_length > size_limit {
                    warn!(size = content_length, limit = size_limit, "compressed body too large");
                    set_edgee_header(&mut response_parts, "proxy-only(compressed-body-too-large)");
                    set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                    return Ok(build_response(response_parts, response_body));
                }
            }

            // if content-encoding is not supported, return true
            if !["gzip", "deflate", "identity", "br", ""].contains(&encoding.unwrap()) {
                warn!("encoding not supported: {}", encoding.unwrap());
                set_edgee_header(&mut response_parts, "proxy-only(encoding-not-supported)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }

            //** END: Only proxy in some cases

            let proxy_duration = timer_start.elapsed().as_millis();

            // decompress the response body
            let cursor = std::io::Cursor::new(response_body.clone());
            let decompressed_body = match encoding {
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
            if routing_ctx.rule.max_decompressed_body_size.is_some() {
                let size_limit = routing_ctx.rule.max_decompressed_body_size.unwrap() as usize;
                if decompressed_body.len() > size_limit {
                    warn!(size = decompressed_body.len(), limit = size_limit, "decompressed body too large");
                    set_edgee_header(&mut response_parts, "compute-aborted(decompressed-body-too-large)");
                    set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                    return Ok(build_response(response_parts, response_body));
                }
            }

            if !decompressed_body.contains(r#"id="__EDGEE_SDK__""#) {
                set_edgee_header(&mut response_parts, "compute-aborted(no-sdk)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }

            //** START : Only compute in some cases

            // if the response has a prefetch purpose, just return the response
            let purpose = response_headers
                .get("purpose")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            let sec_purpose = response_headers
                .get("sec-purpose")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            if purpose.contains("prefetch") || sec_purpose.contains("prefetch") {
                set_edgee_header(&mut response_parts, "compute-aborted(prefetch)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }

            // if the disableEdgeDataCollection query parameter is set, just return the response
            let query = incoming_path.query().unwrap_or("");
            if query.contains("disableEdgeDataCollection") {
                set_edgee_header(&mut response_parts, "compute-aborted(disableEdgeDataCollection)");
                set_duration_headers(&mut response_parts, is_debug_mode, timer_start.elapsed().as_millis(), None);
                return Ok(build_response(response_parts, response_body));
            }

            // todo if the response is cacheable, just return the response

            //** END : Only compute in some cases

            // process the document
            let mut document = html::parse_html(&decompressed_body);

            // if user has no cookie, stop here without processing the payload, else process the payload
            let cookie = edgee_cookie::get(&incoming_headers, &mut HeaderMap::new(), &incoming_host);
            if cookie.is_none() {
                set_edgee_header(&mut response_parts, "compute-aborted(no-cookie)");
            } else {
                let payload = data_collection::process_document(
                    &document,
                    &cookie.unwrap(),
                    proto,
                    &incoming_host,
                    &incoming_path,
                    &response_headers,
                    remote_addr,
                );

                let uuid = payload.uuid.clone();
                if let Err(err) = destinations::send_data_collection(payload).await {
                    warn!(?err, "failed to send data collection payload");
                }
                document.trace_uuid = uuid;
                set_edgee_header(&mut response_parts, "compute");
            }

            // amend the document & the headers
            let mut page_event_param = r#" data-page-event="true""#;
            let event_path = path::generate(incoming_host.as_str());
            let event_path_param = format!(r#" data-event-path="{}""#, event_path);

            if !document.trace_uuid.is_empty() {
                response_parts.headers.insert(
                    HeaderName::from_str("x-edgee-trace").unwrap(),
                    HeaderValue::from_str(&document.trace_uuid).unwrap(),
                );
                page_event_param = r#" data-page-event="false""#;
            }

            let new_body = if !document.inlined_sdk.is_empty() {
                let new_tag = format!(
                    r#"<script{}{}>{}</script>"#,
                    page_event_param,
                    event_path_param,
                    document.inlined_sdk.as_str()
                );
                decompressed_body.replace(document.sdk_full_tag.as_str(), new_tag.as_str())
            } else {
                let new_tag = format!(
                    r#"<script{}{} async src="{}"></script>"#,
                    page_event_param,
                    event_path_param,
                    document.sdk_src.as_str()
                );
                decompressed_body.replace(document.sdk_full_tag.as_str(), new_tag.as_str())
            };

            let data = match encoding {
                Some("gzip") => {
                    let mut encoder = gzip::Encoder::new(Vec::new())?;
                    encoder.write_all(new_body.as_bytes())?;
                    encoder.finish().into_result()?
                }
                Some("deflate") => {
                    let mut encoder = deflate::Encoder::new(Vec::new());
                    encoder.write_all(new_body.as_bytes())?;
                    encoder.finish().into_result()?
                }
                Some("br") => { // handle brotli encoding
                    // q: quality (range: 0-11), lgwin: window size (range: 10-24)
                    let mut encoder  = CompressorWriter::new(Vec::new(), 4096, 11, 24);
                    encoder.write_all(new_body.as_bytes())?;
                    encoder.flush()?;
                    encoder.into_inner()
                }
                Some(_) | None => new_body.into(),
            };

            let full_duration = timer_start.elapsed().as_millis();
            let compute_duration = full_duration - proxy_duration;
            set_duration_headers(&mut response_parts, is_debug_mode, full_duration, Some(compute_duration));

            Ok(build_response(response_parts, Bytes::from(data)))
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
fn set_duration_headers(response_parts: &mut Parts, is_debug_mode: bool, full_duration: u128, compute_duration: Option<u128>) {
    if is_debug_mode {
        response_parts.headers.insert(
            HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
            HeaderValue::from_str(format!("{}ms", full_duration).as_str()).unwrap(),
        );
    }
    if let Some(duration) = compute_duration {
        response_parts.headers.insert(
            HeaderName::from_str(EDGEE_COMPUTE_DURATION_HEADER).unwrap(),
            HeaderValue::from_str(format!("{}ms", duration).as_str()).unwrap(),
        );
        if is_debug_mode {
            let proxy_duration = full_duration - duration;
            response_parts.headers.insert(
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
/// * `response_parts` - A mutable reference to the response parts.
/// * `process` - A string slice representing the process to be set in the header.
///
/// # Logic
///
/// The function inserts the process information into the response headers.
/// The function also logs the process information using the `debug!` macro.
fn set_edgee_header(response_parts: &mut Parts, process: &str) {
    response_parts.headers.insert(
        HeaderName::from_str(EDGEE_HEADER).unwrap(),
        HeaderValue::from_str(process).unwrap(),
    );
    debug!(process);
}

fn error_bad_gateway() -> Response {
    static HTML: &str = include_str!("../public/502.html");
    http::Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Full::from(Bytes::from(HTML)).boxed())
        .expect("response builder should never fail")
}

fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

fn serve_sdk(path: &str) -> anyhow::Result<Response> {
    let inlined_sdk = html::get_sdk_from_url(path);
    if inlined_sdk.is_ok() {
        Ok(http::Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/javascript; charset=utf-8")
            .header(CACHE_CONTROL, "public, max-age=300")
            .body(Full::from(Bytes::from(inlined_sdk.unwrap())).boxed())
            .expect("serving sdk should never fail"))
    } else {
        Ok(http::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(CACHE_CONTROL, "public, max-age=300")
            .body(Full::from(Bytes::from("Not found")).boxed())
            .expect("serving sdk should never fail"))
    }
}

fn build_response(mut parts: http::response::Parts, body: Bytes) -> Response {
    // Update Content-Length header to correct size
    parts.headers.insert("content-length", body.len().into());

    let mut builder = http::Response::builder();
    for (name, value) in parts.headers {
        if name.is_some() {
            builder = builder.header(name.unwrap(), value);
        }
    }
    builder
        .status(parts.status)
        .version(parts.version)
        .extension(parts.extensions)
        .body(Full::from(body).boxed())
        .unwrap()
}

fn preferred_language(headers: &HeaderMap) -> String {
    let accept_language_header = headers
        .get(ACCEPT_LANGUAGE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    let languages = accept_language_header.split(",");
    let lang = "en-us".to_string();
    for l in languages {
        let lang = l.split(";").next().unwrap_or("").trim();
        if !lang.is_empty() {
            return lang.to_lowercase();
        }
    }
    lang
}
