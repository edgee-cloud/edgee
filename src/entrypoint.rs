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
        ACCEPT_LANGUAGE, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, COOKIE,
        REFERER, SET_COOKIE, USER_AGENT,
    },
    HeaderMap, HeaderName, HeaderValue, Method, StatusCode,
};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::body::Incoming;
use incoming_context::IncomingContext;
use libflate::{deflate, gzip};
use proxy_context::ProxyContext;
use routing_context::RoutingContext;
use tracing::{debug, error, warn};

use crate::{
    cookie,
    data_collection::{self, Session},
    destinations, path, real_ip,
};
use crate::{data_collection::Payload, html};

mod incoming_context;
mod proxy_context;
mod routing_context;
mod web;
mod websecure;

type Response = http::Response<BoxBody<Bytes, Infallible>>;

pub async fn start() -> anyhow::Result<()> {
    tokio::select! {
        Err(err) = web::start() => {
            error!(?err, "Failed to start HTTPS entrypoint");
            Err(err)
        }
        Err(err) = websecure::start() => {
            error!(?err, "Failed to start HTTPS entrypoint");
            Err(err)
        }
    }
}

async fn handle_request(
    request: http::Request<Incoming>,
    remote_addr: SocketAddr,
    proto: &str,
) -> anyhow::Result<Response> {
    let timer_start = std::time::Instant::now();
    let incoming_ctx = IncomingContext::new(request, remote_addr, proto == "https");

    let is_debug_mode = match incoming_ctx.header("edgee-debug") {
        Some(_) => true,
        None => false,
    };

    let routing_ctx = match RoutingContext::from_request_context(&incoming_ctx) {
        None => return Ok(error_bad_gateway()),
        Some(r) => r,
    };

    let content_type = incoming_ctx.header(CONTENT_TYPE).unwrap_or(String::new());
    let incoming_method = incoming_ctx.method().clone();
    let incoming_host = incoming_ctx.host().clone();
    let incoming_path = incoming_ctx.path().clone();
    let incoming_headers = incoming_ctx.headers().clone();

    if incoming_method == Method::GET
        && (incoming_path == "/_edgee/sdk.js" || incoming_path == "/_edgee/libs/edgee.v.1.0.0.js")
    {
        debug!(?incoming_path, "serving sdk");
        return serve_sdk(incoming_path.as_str());
    }

    if incoming_method == Method::POST
        && incoming_path == "/_edgee/event"
        && content_type == "application/json"
    {
        let mut res = http::Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(CACHE_CONTROL, "private, no-store")
            .body(empty())
            .unwrap();

        let edgee_cookie = cookie::get_or_create(
            &incoming_headers,
            res.headers_mut(),
            &incoming_host,
            proto == "https",
        );

        let body = incoming_ctx.incoming_body.collect().await?.to_bytes();
        let result: Result<Payload, _> = serde_json::from_reader(body.reader());
        return match result {
            Ok(mut payload) => {
                payload.uuid = uuid::Uuid::new_v4().to_string();
                payload.timestamp = chrono::Utc::now();

                let user_id = edgee_cookie.id.to_string();
                payload.identify.edgee_id = user_id.clone();
                payload.session = Session {
                    session_id: edgee_cookie.ss.timestamp().to_string(),
                    previous_session_id: edgee_cookie
                        .ps
                        .map(|t| t.timestamp().to_string())
                        .unwrap_or_default(),
                    session_count: edgee_cookie.sc,
                    session_start: edgee_cookie.ss == edgee_cookie.ls,
                    first_seen: edgee_cookie.fs,
                    last_seen: edgee_cookie.ls,
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

                payload.client.ip = real_ip::get(remote_addr, &incoming_headers);
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

    let proxy_ctx = ProxyContext::new(incoming_ctx, &routing_ctx);
    let res = proxy_ctx.response().await;

    match res {
        Err(err) => {
            error!(?err, "backend request failed");
            Ok(error_bad_gateway())
        }
        Ok(upstream) => {
            const EDGEE_PROCESS_HEADER: &str = "x-edgee-process";
            const EDGEE_FULL_DURATION_HEADER: &str = "x-edgee-full-duration";
            const EDGEE_COMPUTE_DURATION_HEADER: &str = "x-edgee-compute-duration";
            const EDGEE_PROXY_DURATION_HEADER: &str = "x-edgee-proxy-duration";

            let (mut response_parts, incoming) = upstream.into_parts();
            let response_body = incoming.collect().await?.to_bytes();
            let response_headers = response_parts.headers.clone();
            let encoding = response_headers
                .get(CONTENT_ENCODING)
                .and_then(|h| h.to_str().ok());
            let content_type = response_headers
                .get(CONTENT_TYPE)
                .and_then(|h| h.to_str().ok());
            let content_length = response_headers
                .get(CONTENT_LENGTH)
                .and_then(|h| h.to_str().ok());

            debug!(
                ?encoding,
                ?content_type,
                ?content_length,
                "upstream response"
            );

            if incoming_method == Method::HEAD
                || incoming_method == Method::OPTIONS
                || incoming_method == Method::TRACE
                || incoming_method == Method::CONNECT
            {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(method)").unwrap(),
                    );

                    let elapsed_time = &format!("{}ms", timer_start.elapsed().as_millis());
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
                        HeaderValue::from_str(&elapsed_time).unwrap(),
                    );
                }
                debug!(?incoming_method, "proxy only: contentless method");
                return Ok(build_response(response_parts, response_body));
            }

            if response_parts.status.is_redirection() {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(3xx").unwrap(),
                    );

                    let elapsed_time = &format!("{}ms", timer_start.elapsed().as_millis());
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
                        HeaderValue::from_str(&elapsed_time).unwrap(),
                    );
                }
                debug!(status=?response_parts.status, "proxy only: redirection");
                return Ok(build_response(response_parts, response_body));
            }

            if response_parts.status.is_informational() {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(1xx)").unwrap(),
                    );

                    let elapsed_time = &format!("{}ms", timer_start.elapsed().as_millis());
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
                        HeaderValue::from_str(&elapsed_time).unwrap(),
                    );
                }
                debug!(status=?response_parts.status, "proxy only: informational");
                return Ok(build_response(response_parts, response_body));
            }

            if response_body.is_empty() {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(no-body)").unwrap(),
                    );

                    let elapsed_time = &format!("{}ms", timer_start.elapsed().as_millis());
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
                        HeaderValue::from_str(&elapsed_time).unwrap(),
                    );
                }
                debug!("proxy only: no body");
                return Ok(build_response(response_parts, response_body));
            }

            if content_type.is_none() {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(no-content-type)").unwrap(),
                    );

                    let elapsed_time = &format!("{}ms", timer_start.elapsed().as_millis());
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
                        HeaderValue::from_str(&elapsed_time).unwrap(),
                    );
                }
                debug!(?content_type, "proxy only: no content type");
                return Ok(build_response(response_parts, response_body));
            }

            if !content_type.unwrap().starts_with("text/html") {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(non-html)").unwrap(),
                    );

                    let elapsed_time = &format!("{}ms", timer_start.elapsed().as_millis());
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
                        HeaderValue::from_str(&elapsed_time).unwrap(),
                    );
                }
                debug!(?content_type, "proxy only: not html");
                return Ok(build_response(response_parts, response_body));
            }

            if content_length.is_some() && routing_ctx.rule.max_compressed_body_size.is_some() {
                let content_length = content_length.and_then(|s| s.parse::<u64>().ok()).unwrap();
                let size_limit = routing_ctx.rule.max_compressed_body_size.unwrap();
                if is_debug_mode && content_length > size_limit {
                    warn!(
                        size = content_length,
                        limit = size_limit,
                        "compressed body too large"
                    );
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(compressed-body-too-large)").unwrap(),
                    );

                    let elapsed_time = &format!("{}ms", timer_start.elapsed().as_millis());
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
                        HeaderValue::from_str(&elapsed_time).unwrap(),
                    );
                }
                debug!(?content_length, "proxy only: compressed body too large");
                return Ok(build_response(response_parts, response_body));
            }

            let timer_proxy = timer_start.elapsed().as_millis();

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
                Some(_) | None => String::from_utf8_lossy(&response_body).to_string(),
            };

            if routing_ctx.rule.max_decompressed_body_size.is_some() {
                let size_limit = routing_ctx.rule.max_decompressed_body_size.unwrap() as usize;
                if decompressed_body.len() > size_limit {
                    warn!(
                        size = decompressed_body.len(),
                        limit = size_limit,
                        "decompressed body too large"
                    );
                    if is_debug_mode {
                        response_parts.headers.insert(
                            HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                            HeaderValue::from_str("compute-aborted(decompressed-body-too-large)")
                                .unwrap(),
                        );
                    }
                    debug!(
                        ?content_length,
                        "compute aborted: decompressed body too large"
                    );
                    return Ok(build_response(response_parts, response_body));
                }
            }

            if !decompressed_body.contains(r#"id="__EDGEE_SDK__""#) {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("compute-aborted(no-sdk)").unwrap(),
                    );
                }
                debug!("compute aborted: no sdl");
                return Ok(build_response(response_parts, response_body));
            }

            let purpose = response_headers
                .get("purpose")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            let sec_purpose = response_headers
                .get("sec-purpose")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            if purpose.contains("prefetch") || sec_purpose.contains("prefetch") {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("compute-aborted(prefetch)").unwrap(),
                    );
                }
                debug!("compute aborted: prefetch");
                return Ok(build_response(response_parts, response_body));
            }

            let query = incoming_path.query().unwrap_or("");
            if query.contains("disableEdgeAnalytics") {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("compute-aborted(disableEdgeAnalytics)").unwrap(),
                    );
                }
                debug!("compute aborted: disable edge analytics");
                return Ok(build_response(response_parts, response_body));
            }

            let mut document = html::parse_html(&decompressed_body);

            let cookies = response_headers
                .get(COOKIE)
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            if cookies == "" {
                if is_debug_mode {
                    response_parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("compute-aborted(no-cookie)").unwrap(),
                    );
                }

                debug!("compute aborted: no cookie");
                return Ok(build_response(response_parts, response_body));
            } else {
                let (cookie_str, edgee_cookie) =
                    cookie::get(&incoming_host, proto == "https", cookies);
                response_parts
                    .headers
                    .insert(SET_COOKIE, HeaderValue::from_str(&cookie_str).unwrap());

                let payload = data_collection::process_document(
                    &document,
                    &edgee_cookie,
                    proto,
                    &incoming_host,
                    &incoming_path,
                    &response_headers,
                    remote_addr,
                );

                let uuid = payload.uuid.clone();
                match destinations::send_data_collection(payload).await {
                    Ok(_) => document.trace_uuid = uuid,
                    Err(e) => return Err(e),
                }
            }

            let hostname = routing_ctx
                .backend
                .address
                .split(':')
                .next()
                .unwrap_or(&routing_ctx.backend.address);
            let mut page_event_param = r#" data-page-event="true""#;
            let event_path = path::generate(hostname);
            let event_path_param = format!(r#" data-event-path="{}""#, event_path);

            if !document.trace_uuid.is_empty() {
                response_parts.headers.insert(
                    HeaderName::from_str("x-edgee-analytics-trace").unwrap(),
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
                Some(_) | None => new_body.into(),
            };

            let timer_end = timer_start.elapsed().as_millis();
            let timer_compute = timer_end - timer_proxy;

            if is_debug_mode {
                let full_duration = format!("{}ms", timer_end);
                let proxy_duration = format!("{}ms", timer_proxy);

                response_parts.headers.insert(
                    HeaderName::from_str(EDGEE_FULL_DURATION_HEADER).unwrap(),
                    HeaderValue::from_str(&full_duration).unwrap(),
                );

                response_parts.headers.insert(
                    HeaderName::from_str(EDGEE_PROXY_DURATION_HEADER).unwrap(),
                    HeaderValue::from_str(&proxy_duration).unwrap(),
                );
            }

            let compute_duration = format!("{}ms", timer_compute);
            response_parts.headers.insert(
                HeaderName::from_str(EDGEE_COMPUTE_DURATION_HEADER).unwrap(),
                HeaderValue::from_str(&compute_duration).unwrap(),
            );

            Ok(build_response(response_parts, Bytes::from(data)))
        }
    }
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
    static V0: &str = include_str!("../public/sdk.js");
    static V1: &str = include_str!("../public/edgee.v1.0.0.js");
    let body = if path == "/_edgee/sdk.js" { V0 } else { V1 };
    let resp = http::Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/javascript")
        .header(CACHE_CONTROL, "public, max-age=300")
        .body(Full::from(Bytes::from(body)).boxed())
        .expect("serving sdk should never fail");
    Ok(resp)
}

fn build_response(parts: http::response::Parts, body: Bytes) -> Response {
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
