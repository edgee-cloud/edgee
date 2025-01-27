use std::convert::Infallible;
use std::time::Instant;

use bytes::Bytes;
use http::header::SET_COOKIE;
use http::{header, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use http_body_util::{Empty, Full};
use tracing::info;

use super::compute::{self};
use super::context::incoming::{IncomingContext, RequestHandle};
use super::context::redirection::RedirectionContext;
use crate::config;

type Response = http::Response<BoxBody<Bytes, Infallible>>;

pub async fn edgee_client_event(ctx: IncomingContext) -> anyhow::Result<Response> {
    let res = http::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "private, no-store")
        .header("X-Robots-Tag", "noindex, nofollow")
        .body(empty())?;

    let (mut response, _incoming) = res.into_parts();
    let request = &ctx.get_request().clone();
    let body = ctx.body.collect().await?.to_bytes();

    let mut data_collection_events: String = String::new();
    if !body.is_empty() {
        let events = compute::json_handler(&body, request, &mut response, false).await;
        if events.is_some() {
            data_collection_events = events.unwrap();
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
        .header("X-Robots-Tag", "noindex, nofollow")
        .body(empty())?;
    let (mut response, _incoming) = res.into_parts();

    let request = &ctx.get_request().clone();
    let body = ctx.body.collect().await?.to_bytes();

    if !body.is_empty() {
        let events = compute::json_handler(&body, request, &mut response, true).await;

        let all_cookies = response.headers.get_all(SET_COOKIE).iter();

        let mut set_cookie_header = "";
        let mut set_cookie_u_header = "";
        for cookie in all_cookies {
            if cookie
                .to_str()?
                .starts_with(format!("{}=", config::get().compute.cookie_name.as_str()).as_str())
            {
                set_cookie_header = cookie.to_str()?;
            } else if cookie
                .to_str()?
                .starts_with(format!("{}_u=", config::get().compute.cookie_name.as_str()).as_str())
            {
                set_cookie_u_header = cookie.to_str()?;
            }
        }

        if set_cookie_header.is_empty() {
            return Ok(build_response(response, Bytes::new()));
        }

        let cookie_encrypted = set_cookie_header
            .split(&format!("{}=", config::get().compute.cookie_name.as_str()))
            .nth(1)
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .to_string();

        let cookie_encrypted_u = if set_cookie_u_header.is_empty() {
            "".to_string()
        } else {
            set_cookie_u_header
                .split(&format!(
                    "{}_u=",
                    config::get().compute.cookie_name.as_str()
                ))
                .nth(1)
                .unwrap_or("")
                .split(';')
                .next()
                .unwrap_or("")
                .to_string()
        };

        if cookie_encrypted.is_empty() {
            return Ok(build_response(response, Bytes::new()));
        }

        // check if Egdee-Debug header is present in the request
        let mut is_debug = request.is_debug_mode();
        if request.get_header("Edgee-Debug").is_some() {
            is_debug = true;
        }

        if events.is_some() && is_debug {
            return Ok(build_response(
                response,
                Bytes::from(format!(
                    r#"{{"e":"{}", "u":"{}", "events":{}}}"#,
                    cookie_encrypted,
                    cookie_encrypted_u,
                    events.unwrap()
                )),
            ));
        }
        // set json body {e: cookie_encrypted}

        return Ok(build_response(
            response,
            Bytes::from(format!(
                r#"{{"e":"{}", "u":"{}"}}"#,
                cookie_encrypted, cookie_encrypted_u
            )),
        ));
    }

    Ok(build_response(response, Bytes::new()))
}

pub fn empty_json_response() -> anyhow::Result<Response> {
    Ok(http::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "private, no-store")
        .header("X-Robots-Tag", "noindex, nofollow")
        .body(empty())?)
}

pub fn options(allow_methods: &str) -> anyhow::Result<Response> {
    Ok(http::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_METHODS, allow_methods)
        .header(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            "Content-Type, Edgee-Debug",
        )
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

pub fn build_redirection(associated_redirection: &RedirectionContext) -> anyhow::Result<Response> {
    Ok(http::Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(
            header::LOCATION,
            associated_redirection.destination.as_str(),
        )
        .header(header::CONTENT_TYPE, "text/plain")
        .body(empty())
        .expect("response builder should never fail"))
}

pub fn sdk(ctx: IncomingContext) -> anyhow::Result<Response> {
    if let Ok(mut inlined_sdk) = edgee_sdk::get_sdk_from_url(
        ctx.request.get_path().as_str(),
        ctx.request.get_host().as_str(),
    ) {
        inlined_sdk = inlined_sdk.replace("/_edgee/side", "c");
        Ok(http::Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            )
            .header(header::CACHE_CONTROL, "private, no-store")
            .body(Full::from(Bytes::from(inlined_sdk)).boxed())
            .expect("serving sdk should never fail"))
    } else {
        Ok(http::Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CACHE_CONTROL, "public, max-age=60")
            .body(Full::from(Bytes::from("Not found")).boxed())
            .expect("serving sdk should never fail"))
    }
}

pub fn bad_gateway_error(
    request: &RequestHandle,
    timer_start: Instant,
) -> anyhow::Result<Response> {
    static HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/public/502.html"));

    info!(
        "502 - {} {}{} - {}ms",
        request.get_method(),
        request.get_host(),
        request.get_path(),
        timer_start.elapsed().as_millis()
    );

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

    for (name, value) in parts.headers.iter() {
        builder = builder.header(name, value);
    }
    builder
        .status(parts.status)
        .version(parts.version)
        .extension(parts.extensions)
        .body(Full::from(body).boxed())
        .unwrap()
}
