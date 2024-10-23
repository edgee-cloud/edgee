use crate::proxy::compute::compute;
use crate::proxy::compute::html;
use crate::proxy::context::incoming::{IncomingContext, RequestHandle};
use crate::tools::crypto::encrypt;
use crate::tools::edgee_cookie;
use crate::tools::edgee_cookie::EdgeeCookie;
use bytes::Bytes;
use http::{header, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use http_body_util::{Empty, Full};
use std::collections::HashMap;
use std::convert::Infallible;
use std::time::Instant;
use tracing::info;

type Response = http::Response<BoxBody<Bytes, Infallible>>;

pub async fn edgee_client_event(ctx: IncomingContext) -> anyhow::Result<Response> {
    let res = http::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(empty())?;

    let (mut response, _incoming) = res.into_parts();
    let request = &ctx.get_request().clone();
    let body = ctx.body.collect().await?.to_bytes();

    let mut data_collection_events: String = String::new();
    if body.len() > 0 {
        let data_collection_events_res = compute::json_handler(&body, request, &mut response).await;
        if data_collection_events_res.is_some() {
            data_collection_events = data_collection_events_res.unwrap();
        }
    }

    if request.is_debug_mode() {
        response.status = StatusCode::OK;
        return Ok(build_response(
            response,
            Bytes::from(data_collection_events),
        ));
    }

    Ok(build_response(response, Bytes::new()))
}

pub async fn edgee_client_event_from_third_party_sdk(
    ctx: IncomingContext,
) -> anyhow::Result<Response> {
    let res = http::Response::builder()
        .status(StatusCode::OK)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(empty())?;
    let (mut response, _incoming) = res.into_parts();

    let request = &ctx.get_request().clone();
    let body = ctx.body.collect().await?.to_bytes();

    let cookie: EdgeeCookie;

    // get "e" from query string
    let map: HashMap<String, String> = request
        .get_query()
        .split('&')
        .map(|s| s.split('=').collect::<Vec<&str>>())
        .filter(|v| v.len() == 2)
        .map(|v| (v[0].to_string(), v[1].to_string()))
        .collect();
    let e = map.get("e");
    if e.is_none() {
        // user has no id, set a new cookie
        cookie = EdgeeCookie::new();
    } else {
        // user has an id, decrypt it. if decryption fails, set a new cookie
        cookie =
            edgee_cookie::decrypt_and_update(e.unwrap()).unwrap_or_else(|_| EdgeeCookie::new());
    }
    let cookie_str = serde_json::to_string(&cookie).unwrap();
    let cookie_encrypted = encrypt(&cookie_str).unwrap();

    let mut data_collection_events: String = String::new();
    if body.len() > 0 {
        let data_collection_events_res =
            compute::json_handler(&body, &request, &mut response).await;
        if data_collection_events_res.is_some() {
            data_collection_events = data_collection_events_res.unwrap();
        }
    }

    let resp_body: Bytes;
    if request.is_debug_mode() && data_collection_events.len() > 0 {
        resp_body = Bytes::from(format!(
            r#"{{"e":"{}", "events":{}}}"#,
            cookie_encrypted, data_collection_events
        ));
    } else {
        resp_body = Bytes::from(format!(r#"{{"e":"{}"}}"#, cookie_encrypted));
    }

    Ok(build_response(response, resp_body))
}

pub fn options(allow_methods: &str) -> anyhow::Result<Response> {
    Ok(http::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_METHODS, allow_methods)
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(header::ACCESS_CONTROL_MAX_AGE, "3600")
        .body(empty())
        .expect("response builder should never fail"))
}

pub fn redirect_to_https(request: &RequestHandle) -> anyhow::Result<Response> {
    Ok(http::Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(
            header::LOCATION,
            format!(
                "https://{}{}",
                request.get_host(),
                request.get_path_and_query()
            ),
        )
        .header(header::CONTENT_TYPE, "text/plain")
        .body(empty())
        .expect("response builder should never fail"))
}

pub fn sdk(path: &str) -> anyhow::Result<Response> {
    let inlined_sdk = html::get_sdk_from_url(path);
    if inlined_sdk.is_ok() {
        Ok(http::Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            )
            .header(header::CACHE_CONTROL, "public, max-age=300")
            .body(Full::from(Bytes::from(inlined_sdk.unwrap())).boxed())
            .expect("serving sdk should never fail"))
    } else {
        Ok(http::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CACHE_CONTROL, "public, max-age=300")
            .body(Full::from(Bytes::from("Not found")).boxed())
            .expect("serving sdk should never fail"))
    }
}

pub fn bad_gateway_error(
    request: &RequestHandle,
    timer_start: Instant,
) -> anyhow::Result<Response> {
    info!(
        "502 - {} {}{} - {}ms",
        request.get_method(),
        request.get_host(),
        request.get_path(),
        timer_start.elapsed().as_millis()
    );
    static HTML: &str = include_str!("../../../public/502.html");
    Ok(http::Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Full::from(Bytes::from(HTML)).boxed())
        .expect("response builder should never fail"))
}

fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

pub fn build_response(mut parts: http::response::Parts, body: Bytes) -> Response {
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
