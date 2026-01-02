use std::str::FromStr;

use bytes::Bytes;
use data_collection::{process_from_html, process_from_json};
use http::header::{CACHE_CONTROL, ETAG, EXPIRES, LAST_MODIFIED};
use http::response::Parts;
use http::{HeaderName, HeaderValue};

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
            HeaderValue::from_str("private, no-store").unwrap(),
        );
        response.headers.remove(EXPIRES);
        response.headers.remove(ETAG);
        response.headers.remove(LAST_MODIFIED);
    }

    match do_process_payload(request, response) {
        Ok(_) => {
            if !edgee_cookie::has_cookie(request) {
                set_edgee_header(response, "compute-aborted(no-cookie)");
            } else {
                let events = process_from_html(&document, request, response).await;
                if let Some(events) = events {
                    document.data_collection_events = events;
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

    // do not process the payload if the request is made for specific fetch destination
    let set_fetch_dest = request
        .get_header("sec-fetch-dest")
        .unwrap_or("".to_string());
    if !set_fetch_dest.is_empty() {
        let forbidden_fetch_dest = [
            "audio", "font", "image", "manifest", "script", "style", "video", "empty",
        ];
        if forbidden_fetch_dest.contains(&set_fetch_dest.as_str()) {
            Err("compute-aborted(fetch-dest)")?;
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::init_test_config;
    use core::panic;
    use http::header::{HeaderMap, COOKIE};
    use pretty_assertions::assert_eq;

    fn sample_html_full_minimal() -> String {
        String::from(
            "<html>
            <head>
                <title>ABC > DEF</title>
                <!-- LEGACY STUFF HERE -->
                <link rel=\"canonical\" href=\"https://test.com/test\"/>
                <meta name=\"keywords\" content=\"k1, k2, k3\"/>
                <script type=\"json\" id=\"__EDGEE_DATA_LAYER__\">{
                    \"data_collection\": {
                        \"events\": [
                          {
                            \"type\": \"track\",
                            \"data\": {\"name\": \"Event > name\"}
                          },
                          {
                            \"type\": \"page\",
                            \"data\": {\"name\": \"Page name\", \"title\": \"Page title\"}
                          },
                          {
                            \"type\": \"user\",
                            \"data\": {\"user_id\": \"123\", \"anonymous_id\": \"456\"}
                          }
                        ]
                    }
                }</script>
                <script type=\"javascript\" id=\"__EDGEE_SDK__\" src=\"/_edgee/sdk.js\"/>
            </head>
            <body></body>
        </html>",
        )
    }

    fn sample_html_commented_sdk() -> String {
        String::from(
            "<html>
            <head>
                <title>ABC > DEF</title>
                <!-- <script type=\"javascript\" id=\"__EDGEE_SDK__\" src=\"/_edgee/sdk.js\"/> -->
            </head>
            <body></body>
        </html>",
        )
    }

    fn empty_parts() -> Parts {
        let response = http::response::Builder::new().status(200).body("").unwrap();
        let (parts, _body) = response.into_parts();
        parts
    }

    #[tokio::test]
    async fn html_handler_with_sample_body() {
        init_test_config();
        let body_str = sample_html_full_minimal();
        let request = RequestHandle::default();
        let mut response = empty_parts();

        match html_handler(&body_str, &request, &mut response).await {
            Ok(document) => {
                assert_eq!(document.title, "ABC > DEF");
                assert_eq!(document.canonical, "https://test.com/test");
                assert_eq!(document.keywords, "k1, k2, k3");
                // add checks
            }
            Err(reason) => {
                panic!("Error: {}", reason);
            }
        }
    }

    #[tokio::test]
    async fn html_handler_without_sdk() {
        init_test_config();
        let body_str = "X".repeat(1000);
        let request = RequestHandle::default();
        let mut response = empty_parts();

        match html_handler(&body_str, &request, &mut response).await {
            Ok(_document) => {
                panic!("Should have failed");
            }
            Err(reason) => {
                assert_eq!(reason, "compute-aborted(no-sdk)");
            }
        }
    }

    #[tokio::test]
    async fn html_handler_with_commented_sdk() {
        init_test_config();
        let body_str = sample_html_commented_sdk();
        let request = RequestHandle::default();
        let mut response = empty_parts();

        match html_handler(&body_str, &request, &mut response).await {
            Ok(_document) => {
                panic!("Should have failed");
            }
            Err(reason) => {
                assert_eq!(reason, "compute-aborted(commented-sdk)");
            }
        }
    }

    #[tokio::test]
    async fn html_handler_with_request_cookie() {
        init_test_config();
        let body_str = sample_html_full_minimal();
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, "edgee=abc".parse().unwrap());
        let request = RequestHandle::default_with_headers(headers);
        let mut response = empty_parts();

        match html_handler(&body_str, &request, &mut response).await {
            Ok(document) => {
                assert_eq!(document.title, "ABC > DEF");
                assert_eq!(
                    document.data_collection_events.contains("Event > name"),
                    true
                );
                // add checks
            }
            Err(reason) => {
                panic!("Error: {}", reason);
            }
        }
    }
}
