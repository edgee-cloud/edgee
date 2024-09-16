use bytes::Bytes;
use crate::config::config;
use crate::proxy::compute::html::{parse_html, Document};
use crate::proxy::proxy::set_edgee_header;
use crate::tools::edgee_cookie;
use crate::proxy::compute::data_collection::data_collection;
use http::response::Parts;
use http::uri::PathAndQuery;
use http::HeaderMap;
use tracing::{warn};
use crate::tools::edgee_cookie::EdgeeCookie;

pub async fn html_handler(
    body: &String,
    host: &String,
    path: &PathAndQuery,
    request_headers: &HeaderMap,
    proto: &str,
    client_ip: &String,
    response_parts: &mut Parts,
    response_headers: &HeaderMap,
) -> Result<Document, &'static str>
{
    // if the decompressed body is too large, abort the computation
    if body.len() > config::get().compute.max_decompressed_body_size {
        warn!("decompressed body too large: {} > {}", body.len(), config::get().compute.max_decompressed_body_size);
        Err("compute-aborted(decompressed-body-too-large)")?;
    }

    // check if `id="__EDGEE_SDK__"` is present in the body
    if !body.contains(r#"id="__EDGEE_SDK__""#) {
        Err("compute-aborted(no-sdk)")?;
    }

    let mut document = parse_html(&body);
    match do_process_payload(&path, request_headers, response_headers) {
        Ok(_) => {
            let cookie = edgee_cookie::get(&request_headers, &mut HeaderMap::new(), &host);
            if cookie.is_none() {
                set_edgee_header(response_parts, "compute-aborted(no-cookie)");
            } else {
                let data_collection_trace_uuid = data_collection::process_from_html(&document, &cookie.unwrap(), proto, &host, &path, &request_headers, client_ip).await;
                if data_collection_trace_uuid.is_some() {
                    document.trace_uuid = data_collection_trace_uuid.unwrap();
                }
            }
        }
        Err(reason) => {
            set_edgee_header(response_parts, reason);
        }
    }

    Ok(document)
}

pub async fn json_handler(body: &Bytes, cookie: &EdgeeCookie, path: &PathAndQuery, request_headers: &HeaderMap, client_ip: &String) {
    data_collection::process_from_json(body, &cookie, &path, &request_headers, client_ip).await;
}

/// Processes the payload of a request under certain conditions.
///
/// This function checks for several conditions before processing the payload of a request.
/// If any of these conditions are met, the function will abort the computation and return an error.
///
/// # Arguments
///
/// * `path` - A reference to the path
/// * `response_headers` - A reference to the response headers.
///
/// # Returns
///
/// * `Result<bool, &'static str>` - Returns a Result. If the payload is processed successfully, it returns `Ok(true)`.
/// If any of the conditions are met, it returns `Err` with a string indicating the reason for the computation abort.
///
/// # Errors
///
/// This function will return an error if:
///
/// * The `disableEdgeDataCollection` query parameter is present in the URL of the request.
/// * The response is cacheable.
/// * The request is for prefetch (indicated by the `Purpose` or `Sec-Purpose` headers).
fn do_process_payload(path: &PathAndQuery, request_headers: &HeaderMap, response_headers: &HeaderMap) -> Result<bool, &'static str> {
    // do not process the payload if disableEdgeDataCollection query param is present in the URL
    let query = path.query().unwrap_or("");
    if query.contains("disableEdgeDataCollection") {
        Err("compute-aborted(disableEdgeDataCollection)")?;
    }

    // process the payload, only if response is not cacheable
    if is_cacheable(response_headers) {
        Err("compute-aborted(cacheable)")?;
    }

    // do not process the payload if the request is for prefetch
    let purpose = request_headers.get("purpose").and_then(|h| h.to_str().ok()).unwrap_or("");
    let sec_purpose = request_headers.get("sec-purpose").and_then(|h| h.to_str().ok()).unwrap_or("");
    if purpose.contains("prefetch") || sec_purpose.contains("prefetch") {
        Err("compute-aborted(prefetch)")?;
    }

    Ok(true)
}

/// Determines if a response is cacheable based on the configuration and response headers.
///
/// This function first checks if the `behind_proxy_cache` configuration is set to true.
/// If it is, it calls the `is_cacheable_by_cdn_or_browser` function to determine if the response is cacheable.
/// If the `edgee_behind_proxy_cache` configuration is not set to true, it calls the `is_cacheable_by_browser` function.
///
/// # Arguments
///
/// * `response_headers` - A reference to the response headers.
///
/// # Returns
///
/// * `bool` - Returns a boolean indicating if the response is cacheable.
fn is_cacheable(response_headers: &HeaderMap) -> bool {
    if config::get().compute.behind_proxy_cache {
        return is_cacheable_by_cdn_or_browser(response_headers);
    }
    is_cacheable_by_browser(response_headers)
}

/// Determines if a response is cacheable by a browser based on the response headers.
///
/// This function checks the `Cache-Control`, `Expires`, `Last-Modified`, and `Etag` headers of the response.
/// It uses these headers to determine if the response is cacheable by a browser.
///
/// # Arguments
///
/// * `response_headers` - A reference to the response headers.
///
/// # Returns
///
/// * `bool` - Returns a boolean indicating if the response is cacheable by a browser.
///
/// # Cacheability conditions
///
/// The function considers a response cacheable if:
///
/// * The `Etag`, `Last-Modified`, or `Expires` headers are not empty.
/// * The `Cache-Control` header contains `public`, `max-age`, or `no-cache`.
///
/// The function considers a response not cacheable if:
///
/// * The `Cache-Control` header contains `private` and `no-store`.
/// * The `Cache-Control` header contains `public` and `max-age=0`.
fn is_cacheable_by_browser(response_headers: &HeaderMap) -> bool {
    let cache_control = response_headers.get("Cache-Control").map_or("", |v| v.to_str().unwrap());
    let expires = response_headers.get("Expires").map_or("", |v| v.to_str().unwrap());
    let last_modified = response_headers.get("Last-Modified").map_or("", |v| v.to_str().unwrap());
    let etag = response_headers.get("Etag").map_or("", |v| v.to_str().unwrap());

    if cache_control.contains("private") && cache_control.contains("no-store") {
        return false;
    }

    if etag != "" || last_modified != "" || expires != "" {
        return true;
    }

    if cache_control.contains("public") && cache_control.contains("max-age=0") {
        return false;
    }

    if cache_control.contains("public") || cache_control.contains("max-age") || cache_control.contains("no-cache") {
        return true;
    }

    false
}

/// Determines if a response is cacheable by a CDN or a browser based on the response headers.
///
/// This function checks the `Cache-Control`, `Surrogate-Control`, `Expires`, `Last-Modified`, and `Etag` headers of the response.
/// It uses these headers to determine if the response is cacheable by a CDN or a browser.
///
/// # Arguments
///
/// * `response_headers` - A reference to the response headers.
///
/// # Returns
///
/// * `bool` - Returns a boolean indicating if the response is cacheable by a CDN or a browser.
///
/// # Cacheability conditions
///
/// The function considers a response cacheable if:
///
/// * The `Etag`, `Last-Modified`, or `Expires` headers are not empty.
///
/// The function considers a response not cacheable if:
///
/// * The `Surrogate-Control` header contains `private` and `no-store`.
/// * The `Cache-Control` header contains `private` and `no-store`.
/// * The `Cache-Control` header contains `public` and `max-age=0`.
/// * The `Cache-Control` header contains `private` and `max-age=0`.
fn is_cacheable_by_cdn_or_browser(response_headers: &HeaderMap) -> bool {
    let cache_control = response_headers.get("Cache-Control").map_or("", |v| v.to_str().unwrap());
    let surrogate_control = response_headers.get("Surrogate-Control").map_or("", |v| v.to_str().unwrap());
    let expires = response_headers.get("Expires").map_or("", |v| v.to_str().unwrap());
    let last_modified = response_headers.get("Last-Modified").map_or("", |v| v.to_str().unwrap());
    let etag = response_headers.get("Etag").map_or("", |v| v.to_str().unwrap());

    if surrogate_control.contains("private") && surrogate_control.contains("no-store") {
        return false;
    }

    if cache_control.contains("private") && cache_control.contains("no-store") {
        return false;
    }

    if etag != "" || last_modified != "" || expires != "" {
        return true;
    }

    if cache_control.contains("public") && cache_control.contains("max-age=0") {
        return false;
    }

    if cache_control.contains("private") && cache_control.contains("max-age=0") {
        return false;
    }

    true
}
