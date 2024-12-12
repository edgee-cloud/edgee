use std::str::FromStr;

use bytes::Bytes;
use data_collection::{process_from_html, process_from_json};
use http::header::CACHE_CONTROL;
use http::response::Parts;
use http::{HeaderName, HeaderValue};
use tracing::warn;

use crate::config;
use crate::proxy::context::incoming::RequestHandle;
use crate::proxy::set_edgee_header;
use crate::tools::{
    self,
    edgee_cookie::{self},
};
use html::{parse_html, Document};

pub mod data_collection;
pub mod html;

pub async fn html_handler(
    body: &str,
    request: &RequestHandle,
    response: &mut Parts,
) -> Result<Document, &'static str> {
    // if the decompressed body is too large, abort the computation
    if body.len() > config::get().compute.max_decompressed_body_size {
        warn!(
            "decompressed body too large: {} > {}",
            body.len(),
            config::get().compute.max_decompressed_body_size
        );
        Err("compute-aborted(decompressed-body-too-large)")?;
    }

    // check if `id="__EDGEE_SDK__"` is present in the body
    if !body.contains(r#"id="__EDGEE_SDK__""#) {
        Err("compute-aborted(no-sdk)")?;
    }

    let mut document = parse_html(body, request.get_host().as_str());

    // verify if document.sdk_full_tag is present, otherwise SDK is probably commented in the page
    if document.sdk_full_tag.is_empty() {
        Err("compute-aborted(commented-sdk)")?;
    }

    // enforce_no_store_policy is used to enforce no-store cache-control header in the response for requests that can be computed
    if config::get().compute.enforce_no_store_policy {
        response.headers.insert(
            HeaderName::from_str(CACHE_CONTROL.as_ref()).unwrap(),
            HeaderValue::from_str("no-store").unwrap(),
        );
    }

    match do_process_payload(request, response) {
        Ok(_) => {
            if !edgee_cookie::has_cookie(request) {
                set_edgee_header(response, "compute-aborted(no-cookie)");
            } else {
                let events = process_from_html(&document, request, response).await;
                if events.is_some() {
                    document.data_collection_events = events.unwrap();
                }
            }
        }
        Err(reason) => {
            set_edgee_header(response, reason);
        }
    }

    Ok(document)
}

pub async fn json_handler(
    body: &Bytes,
    request: &RequestHandle,
    response: &mut Parts,
    from_third_party_sdk: bool,
) -> Option<String> {
    process_from_json(body, request, response, from_third_party_sdk).await
}

/// Processes the payload of a request under certain conditions.
///
/// This function checks for several conditions before processing the payload of a request.
/// If any of these conditions are met, the function will abort the computation and return an error.
///
/// # Arguments
///
/// * `request` - A reference to the request object
/// * `response` - A mutable reference to the response parts
///
/// # Returns
///
/// * `Result<bool, &'static str>` - Returns a Result. If the payload is processed successfully, it returns `Ok(true)`.
///   If any of the conditions are met, it returns `Err` with a string indicating the reason for the computation abort.
///
/// # Errors
///
/// This function will return an error if:
///
/// * The `disableEdgeDataCollection` query parameter is present in the URL of the request.
/// * The response is cacheable.
/// * The request is for prefetch (indicated by the `Purpose` or `Sec-Purpose` headers).
fn do_process_payload(request: &RequestHandle, response: &mut Parts) -> Result<bool, &'static str> {
    // do not process the payload if disableEdgeDataCollection query param is present in the URL
    let query = request.get_query().as_str();
    if query.contains("disableEdgeDataCollection") {
        Err("compute-aborted(disableEdgeDataCollection)")?;
    }

    if !config::get().compute.enforce_no_store_policy {
        // process the payload, only if response is not cacheable
        // transform response_headers to HashMap<String, String>
        let res_headers = response
            .headers
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap().to_string()))
            .collect::<std::collections::HashMap<String, String>>();
        if tools::cacheable::check_cacheability(
            &res_headers,
            config::get().compute.behind_proxy_cache,
        ) {
            Err("compute-aborted(cacheable)")?;
        }
    }

    // do not process the payload if the request is for prefetch
    let purpose = request.get_header("purpose").unwrap_or("".to_string());
    let sec_purpose = request.get_header("sec-purpose").unwrap_or("".to_string());
    if purpose.contains("prefetch") || sec_purpose.contains("prefetch") {
        Err("compute-aborted(prefetch)")?;
    }

    Ok(true)
}
