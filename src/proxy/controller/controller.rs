use crate::proxy::compute::compute;
use crate::proxy::compute::html;
use crate::proxy::context::incoming::IncomingContext;
use crate::tools::crypto::encrypt;
use crate::tools::edgee_cookie;
use crate::tools::edgee_cookie::EdgeeCookie;
use bytes::Bytes;
use http::uri::PathAndQuery;
use http::{header, HeaderMap, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use http_body_util::{Empty, Full};
use std::collections::HashMap;
use std::convert::Infallible;

type Response = http::Response<BoxBody<Bytes, Infallible>>;

pub async fn edgee_client_event(
    incoming_ctx: IncomingContext,
    host: &String,
    path: &PathAndQuery,
    request_headers: &HeaderMap,
    client_ip: &String,
) -> anyhow::Result<Response> {
    let res = http::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(empty())?;

    let (mut response_parts, _incoming) = res.into_parts();
    let cookie = edgee_cookie::get_or_set(&request_headers, &mut response_parts, &host);

    let body = incoming_ctx.incoming_body.collect().await?.to_bytes();
    if body.len() > 0 {
        compute::json_handler(&body, &cookie, path, request_headers, client_ip).await;
    }
    Ok(build_response(response_parts, Bytes::new()))
}

pub async fn edgee_client_event_from_third_party_sdk(
    incoming_ctx: IncomingContext,
    path: &PathAndQuery,
    request_headers: &HeaderMap,
    client_ip: &String,
) -> anyhow::Result<Response> {
    let body = incoming_ctx.incoming_body.collect().await?.to_bytes();

    let cookie: EdgeeCookie;

    // get "e" from query string
    let map: HashMap<String, String> = path
        .query()
        .unwrap_or("")
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

    if body.len() > 0 {
        compute::json_handler(&body, &cookie, path, request_headers, client_ip).await;
    }

    Ok(http::Response::builder()
        .status(StatusCode::OK)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Full::from(Bytes::from(format!(r#"{{"e":"{}"}}"#, cookie_encrypted))).boxed())
        .expect("serving sdk should never fail"))
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

pub fn redirect_to_https(
    incoming_host: String,
    incoming_path: PathAndQuery,
) -> anyhow::Result<Response> {
    Ok(http::Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(
            header::LOCATION,
            format!("https://{}{}", incoming_host, incoming_path),
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

pub fn bad_gateway_error() -> anyhow::Result<Response> {
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
