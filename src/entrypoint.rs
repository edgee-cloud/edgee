use std::{
    convert::Infallible,
    io::{Read, Write},
    net::SocketAddr,
    str::FromStr,
};

use anyhow::bail;
use bytes::Bytes;
use http::{
    header::{CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HOST},
    uri::PathAndQuery,
    HeaderName, HeaderValue, Method, StatusCode, Uri,
};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::body::Incoming;
use hyper_rustls::ConfigBuilderExt;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use libflate::{deflate, gzip};
use regex::Regex;
use tracing::{debug, error};

use crate::{config, path};
use crate::{config::RoutingRulesConfiguration, html};

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
    mut req: http::Request<Incoming>,
    remote_addr: SocketAddr,
    proto: &str,
) -> anyhow::Result<Response> {
    let host = match (req.headers().get(HOST), req.uri().host()) {
        (None, Some(value)) => Some(String::from(value)),
        (Some(value), _) => Some(value.to_str().unwrap().to_string()),
        (None, None) => None,
    }
    .and_then(|host| host.split(':').next().map(|s| s.to_string()))
    .expect("host should be available");

    let cfg = &config::get().routing;
    let routing = cfg.iter().find(|r| r.domain == host);

    let routing = match routing {
        None => return Ok(error_bad_gateway()),
        Some(r) => r,
    };

    let has_debug_header = match req.headers().get("edgee-debug") {
        Some(_) => true,
        None => false,
    };

    let method = req.method().clone();
    let headers = req.headers().clone();
    let default_content_type = &HeaderValue::from_str("").unwrap();
    let content_type = headers.get(CONTENT_TYPE).unwrap_or(default_content_type);
    let uri = req.uri_mut();
    let root_path = PathAndQuery::from_str("/").expect("'/' should be a valid path");
    let requested_path = uri.path_and_query().unwrap_or(&root_path);

    if method == Method::GET
        && (requested_path == "/_edgee/sdk.js" || requested_path == "/_edgee/libs/edgee.v.1.0.0.js")
    {
        return serve_sdk(requested_path.as_str());
    }

    // TODO: Process events
    if method == Method::POST && content_type == "application/json" {
        return Ok(error_bad_gateway());
    }

    let default_backend = match routing.backends.iter().find(|b| b.default) {
        Some(a) => a,
        None => return Ok(error_bad_gateway()),
    };

    let mut upstream_backend: Option<&config::BackendConfiguration> = None;
    let mut upstream_path: Option<PathAndQuery> = None;
    let mut current_rule: Option<RoutingRulesConfiguration> = None;
    for rule in routing.rules.clone() {
        current_rule = Some(rule.clone());
        match (rule.path, rule.path_prefix, rule.path_regexp) {
            (Some(path), _, _) => {
                if *requested_path == *path {
                    upstream_backend = match rule.backend {
                        Some(name) => routing.backends.iter().find(|b| b.name == name),
                        None => Some(default_backend),
                    };
                    upstream_path = match rule.rewrite {
                        Some(replacement) => PathAndQuery::from_str(&replacement).ok(),
                        None => PathAndQuery::from_str(&path).ok(),
                    };
                    break;
                }
            }
            (None, Some(prefix), _) => {
                if requested_path.to_string().starts_with(&prefix) {
                    upstream_backend = match rule.backend {
                        Some(name) => routing.backends.iter().find(|b| b.name == name),
                        None => Some(default_backend),
                    };
                    upstream_path = match rule.rewrite {
                        Some(replacement) => {
                            let new_path =
                                requested_path
                                    .to_string()
                                    .replacen(&prefix, &replacement, 1);
                            PathAndQuery::from_str(&new_path).ok()
                        }
                        None => Some(requested_path.clone()),
                    };
                    break;
                }
            }
            (None, None, Some(pattern)) => {
                let regexp = Regex::new(&pattern).expect("regex pattern should be valid");
                let path = requested_path.to_string();
                if regexp.is_match(&path) {
                    upstream_backend = match rule.backend {
                        Some(name) => routing.backends.iter().find(|b| b.name == name),
                        None => Some(default_backend),
                    };
                    upstream_path = match rule.rewrite {
                        Some(replacement) => {
                            PathAndQuery::from_str(&regexp.replacen(&path, 1, &replacement)).ok()
                        }
                        None => PathAndQuery::from_str(&path).ok(),
                    };
                    break;
                }
            }
            _ => bail!("Invalid routing"),
        }
    }

    let backend = upstream_backend.unwrap_or(default_backend);
    let path = upstream_path.unwrap_or(requested_path.clone());
    let current_rule = current_rule.unwrap();

    const FORWARDED_FOR: &str = "x-forwarded-for";
    let client_ip = remote_addr.ip().to_string();
    if let Some(forwarded_for) = req.headers_mut().get_mut(FORWARDED_FOR) {
        let existing_value = forwarded_for.to_str().unwrap();
        let new_value = format!("{}, {}", existing_value, client_ip);
        *forwarded_for = HeaderValue::from_str(&new_value).expect("header value should be valid");
    } else {
        req.headers_mut().insert(
            FORWARDED_FOR,
            HeaderValue::from_str(&client_ip).expect("header value should be valid"),
        );
    }

    const FORWARDED_PROTO: &str = "x-forwarded-proto";
    if req.headers().get(FORWARDED_PROTO).is_none() {
        req.headers_mut().insert(
            FORWARDED_PROTO,
            HeaderValue::from_str(proto).expect("header value should be valid"),
        );
    }

    const FORWARDED_HOST: &str = "x-forwarded-host";
    if req.headers().get(FORWARDED_HOST).is_none() {
        req.headers_mut().insert(
            FORWARDED_HOST,
            HeaderValue::from_str(&host).expect("header value should be valid"),
        );
    }
    let res = if backend.enable_ssl {
        forward_https_request(req, backend, path).await
    } else {
        forward_http_request(req, backend, path).await
    };

    match res {
        Err(err) => {
            error!(?err, "backend request failed");
            Ok(error_bad_gateway())
        }
        Ok(upstream) => {
            const EDGEE_PROCESS_HEADER: &str = "x-edgee-process";

            let (mut parts, incoming) = upstream.into_parts();
            let body = incoming.collect().await?.to_bytes();
            let headers = parts.headers.clone();
            let encoding = headers.get(CONTENT_ENCODING).and_then(|h| h.to_str().ok());
            let content_type = headers.get(CONTENT_TYPE).and_then(|h| h.to_str().ok());
            let content_length = headers.get(CONTENT_LENGTH).and_then(|h| h.to_str().ok());

            if body.is_empty() {
                if has_debug_header {
                    parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(no-body)").unwrap(),
                    );
                }
                return Ok(build_response(parts, body));
            }

            if method == Method::HEAD
                || method == Method::OPTIONS
                || method == Method::TRACE
                || method == Method::CONNECT
            {
                if has_debug_header {
                    parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(method)").unwrap(),
                    );
                }
                return Ok(build_response(parts, body));
            }

            if parts.status.is_redirection() {
                if has_debug_header {
                    parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(3xx").unwrap(),
                    );
                }
                return Ok(build_response(parts, body));
            }

            if parts.status.is_informational() {
                if has_debug_header {
                    parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(1xx)").unwrap(),
                    );
                }
                return Ok(build_response(parts, body));
            }

            if content_type.is_none() {
                if has_debug_header {
                    parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(no-content-type)").unwrap(),
                    );
                }
                return Ok(build_response(parts, body));
            }

            if !content_type.unwrap().starts_with("text/html") {
                if has_debug_header {
                    parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(non-html)").unwrap(),
                    );
                }
                return Ok(build_response(parts, body));
            }

            if content_length.is_some() && current_rule.max_compressed_body_size.is_some() {
                let content_length = content_length.and_then(|s| s.parse::<u64>().ok()).unwrap();
                let size_limit = current_rule.max_compressed_body_size.unwrap();
                if has_debug_header && content_length > size_limit {
                    parts.headers.insert(
                        HeaderName::from_str(EDGEE_PROCESS_HEADER).unwrap(),
                        HeaderValue::from_str("proxy-only(compressed-body-too-large)").unwrap(),
                    );
                }
                return Ok(build_response(parts, body));
            }

            let cursor = std::io::Cursor::new(body.clone());
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
                Some(_) | None => String::from_utf8_lossy(&body).to_string(),
            };

            let new_body = match parse_body(&decompressed_body) {
                Embedding::Empty => decompressed_body,
                Embedding::Doc(document) => {
                    let hostname = backend
                        .address
                        .split(':')
                        .next()
                        .unwrap_or(&backend.address);
                    let mut page_event_param = r#" data-page-event="true""#;
                    let event_path = path::generate(hostname);
                    let event_path_param = format!(r#" data-event-path="{}""#, event_path);

                    if !document.trace_uuid.is_empty() {
                        page_event_param = r#" data-page-event="false""#;
                    }

                    if !document.inlined_sdk.is_empty() {
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
                    }
                }
            };

            match encoding {
                Some("gzip") => {
                    let mut encoder = gzip::Encoder::new(Vec::new())?;
                    encoder.write_all(new_body.as_bytes())?;
                    let data = encoder.finish().into_result()?;
                    Ok(build_response(parts, Bytes::from(data)))
                }
                Some("deflate") => {
                    let mut encoder = deflate::Encoder::new(Vec::new());
                    encoder.write_all(new_body.as_bytes())?;
                    let data = encoder.finish().into_result()?;
                    Ok(build_response(parts, Bytes::from(data)))
                }
                Some(_) | None => Ok(build_response(parts, Bytes::from(new_body))),
            }
        }
    }
}

enum Embedding {
    Empty,
    Doc(html::Document),
}

fn parse_body(body: &str) -> Embedding {
    if !body.contains(r#"id="__EDGEE_SDK__""#) {
        return Embedding::Empty;
    }

    Embedding::Doc(html::parse_html(body))
}

async fn forward_http_request(
    orig: http::Request<Incoming>,
    backend: &config::BackendConfiguration,
    path: PathAndQuery,
) -> anyhow::Result<http::Response<Incoming>> {
    let uri: Uri = format!("http://{}{}", &backend.address, path)
        .parse()
        .expect("uri should be valid");

    debug!(origin=?orig.uri(),?uri, "Forwarding HTTP request");

    let mut req = http::Request::builder().uri(uri).method(orig.method());
    let headers = req.headers_mut().expect("request should have headers");
    for (name, value) in orig.headers().iter() {
        headers.insert(name, value.to_owned());
    }

    headers.insert(
        "host",
        HeaderValue::from_str(&backend.address).expect("host should be valid"),
    );

    let (_parts, body) = orig.into_parts();
    let req = req.body(body).expect("request to be built");
    let client = Client::builder(TokioExecutor::new()).build(HttpConnector::new());
    client
        .request(req)
        .await
        .map_err(|err| anyhow::Error::new(err))
}

async fn forward_https_request(
    mut req: http::Request<Incoming>,
    backend: &config::BackendConfiguration,
    path: PathAndQuery,
) -> anyhow::Result<http::Response<Incoming>> {
    let uri: Uri = format!("https://{}{}", &backend.address, path)
        .parse()
        .expect("uri should be valid");

    *req.uri_mut() = uri;

    req.headers_mut().insert(
        "host",
        HeaderValue::from_str(&backend.address).expect("host should be valid"),
    );

    let client_config = rustls::ClientConfig::builder()
        .with_native_roots()?
        .with_no_client_auth();
    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(client_config)
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();
    let client = Client::builder(TokioExecutor::new()).build(connector);
    client
        .request(req)
        .await
        .map_err(|err| anyhow::Error::new(err))
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

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
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
