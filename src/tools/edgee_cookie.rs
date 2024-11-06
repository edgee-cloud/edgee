use std::collections::HashMap;
use std::io::Read;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use cookie::time::OffsetDateTime;
use cookie::{Cookie, SameSite};
use http::header::{COOKIE, SET_COOKIE};
use http::response::Parts;
use http::{HeaderValue, Method};
use serde::{Deserialize, Serialize};
use serde_json::Error;
use uuid::Uuid;

use super::crypto::{decrypt, encrypt};
use crate::config;
use crate::proxy::compute::data_collection::payload::Payload;
use crate::proxy::context::incoming::RequestHandle;
use crate::proxy::DATA_COLLECTION_ENDPOINT_FROM_THIRD_PARTY_SDK;

static SESSION_DURATION: Duration = Duration::minutes(30);

#[derive(Serialize, Deserialize, Debug)]
pub struct EdgeeCookie {
    pub id: Uuid,          // v4 uuid
    pub fs: DateTime<Utc>, //first seen
    pub ls: DateTime<Utc>, // last seen
    pub ss: DateTime<Utc>, // session start (used to create session_id as well)
    #[serde(skip_serializing)]
    pub ps: Option<DateTime<Utc>>, // previous session
    pub sc: u32,           // session count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sz: Option<String>, // screen size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uaa: Option<String>, // user agent architecture
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uab: Option<String>, // user agent bitness
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uam: Option<String>, // user agent model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uapv: Option<String>, // user agent platform version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uafvl: Option<String>, // user agent full version list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tz: Option<String>, // timezone
}

impl EdgeeCookie {
    pub fn new() -> EdgeeCookie {
        let now = Utc::now();
        EdgeeCookie {
            id: Uuid::new_v4(),
            fs: now,
            ls: now,
            ss: now,
            ps: None,
            sc: 1,
            sz: None,
            uaa: None,
            uab: None,
            uam: None,
            uapv: None,
            uafvl: None,
            tz: None,
        }
    }

    fn set_screen_size(&mut self, payload: &Payload) {
        if let Some(client) = payload
            .data_collection
            .as_ref()
            .and_then(|dc| dc.context.as_ref())
            .and_then(|ctx| ctx.client.as_ref())
        {
            let screen_width = client.screen_width;
            let screen_height = client.screen_height;
            let screen_density = client.screen_density;

            if screen_width.is_none() || screen_height.is_none() || screen_density.is_none() {
                return;
            }

            self.sz = Some(format!(
                "{}x{}x{}",
                screen_width.unwrap(),
                screen_height.unwrap(),
                screen_density.unwrap()
            ));
        }
    }

    fn set_client_hints(&mut self, payload: &Payload) {
        if let Some(client) = payload
            .data_collection
            .as_ref()
            .and_then(|dc| dc.context.as_ref())
            .and_then(|ctx| ctx.client.as_ref())
        {
            if client.user_agent_architecture.is_some() {
                self.uaa = client.user_agent_architecture.clone();
            }
            if client.user_agent_bitness.is_some() {
                self.uab = client.user_agent_bitness.clone();
            }
            if client.user_agent_model.is_some() {
                self.uam = client.user_agent_model.clone();
            }
            if client.os_version.is_some() {
                self.uapv = client.os_version.clone();
            }
            if client.user_agent_full_version_list.is_some() {
                self.uafvl = client.user_agent_full_version_list.clone();
            }
            if client.timezone.is_some() {
                self.tz = client.timezone.clone();
            }
        }
    }
}

/// Retrieves an `EdgeeCookie` from the request headers or initializes a new one if it does not exist,
/// decrypts and updates it, and sets it in the response headers.
///
/// # Arguments
///
/// * `request_headers` - A reference to the request headers.
/// * `response` - A mutable reference to the response headers where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Returns
///
/// * `EdgeeCookie` - The `EdgeeCookie` that was retrieved or newly created.
pub fn get_or_set(request: &RequestHandle, response: &mut Parts, payload: &Payload) -> EdgeeCookie {
    let edgee_cookie = get(request, response, payload);
    if edgee_cookie.is_none() {
        return init_and_set_cookie(request, response, payload);
    }
    edgee_cookie.unwrap()
}

pub fn has_cookie(request: &RequestHandle) -> bool {
    get_cookie(request).is_some()
}

pub fn get_cookie(request: &RequestHandle) -> Option<String> {
    let all_cookies = request.get_headers().get_all(COOKIE);
    for cookie in all_cookies {
        let mut map = HashMap::new();
        for item in cookie.to_str().unwrap().split("; ") {
            let parts: Vec<&str> = item.split('=').collect();
            if parts.len() != 2 {
                continue;
            }
            map.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
        }

        if map.contains_key(config::get().compute.cookie_name.as_str()) {
            return Some(
                map.get(config::get().compute.cookie_name.as_str())
                    .unwrap()
                    .to_string(),
            );
        }
    }
    get_cookie_from_query_or_none(request)
}

fn get_cookie_from_query_or_none(request: &RequestHandle) -> Option<String> {
    if request.get_path() == DATA_COLLECTION_ENDPOINT_FROM_THIRD_PARTY_SDK
        && request.get_method() == Method::POST
    {
        let map: HashMap<String, String> = request
            .get_query()
            .split('&')
            .map(|s| s.split('=').collect::<Vec<&str>>())
            .filter(|v| v.len() == 2)
            .map(|v| (v[0].to_string(), v[1].to_string()))
            .collect();
        let e = map.get("e");
        e.map(|e| e.to_string())
    } else {
        None
    }
}

/// Retrieves an `EdgeeCookie` from the request headers, decrypts and updates it, and sets it in the response headers.
///
/// # Arguments
///
/// * `request` - A reference to the request headers.
/// * `response` - A mutable reference to the response headers where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Returns
///
/// * `Option<EdgeeCookie>` - An `Option` containing the `EdgeeCookie` if it exists and is successfully decrypted and updated, or `None` if the cookie does not exist or decryption fails.
pub fn get(
    request: &RequestHandle,
    response: &mut Parts,
    payload: &Payload,
) -> Option<EdgeeCookie> {
    if let Some(value) = get_cookie(request) {
        let edgee_cookie_result = decrypt_and_update(value.as_str(), payload);
        if edgee_cookie_result.is_err() {
            return Some(init_and_set_cookie(request, response, payload));
        }
        let mut edgee_cookie = edgee_cookie_result.unwrap();
        edgee_cookie.set_screen_size(payload);
        edgee_cookie.set_client_hints(payload);

        let edgee_cookie_str = serde_json::to_string(&edgee_cookie).unwrap();
        let edgee_cookie_encrypted = encrypt(&edgee_cookie_str).unwrap();
        set_cookie(
            &edgee_cookie_encrypted,
            response,
            request.get_host().as_str(),
        );

        return Some(edgee_cookie);
    }
    None
}

/// Decrypts an encrypted `EdgeeCookie`, updates its fields based on the current time, and returns the updated `EdgeeCookie`.
///
/// # Arguments
///
/// * `encrypted_edgee_cookie` - A string slice that holds the encrypted `EdgeeCookie`.
/// * `payload` - A reference to the payload object that contains the data collection information.
///
/// # Returns
///
/// * `Ok(EdgeeCookie)` - A `Result` containing the decrypted and updated `EdgeeCookie` if successful.
/// * `Err(&'static str)` - A `Result` containing an error message if decryption or parsing fails.
///
/// # Errors
///
/// This function will return an error if the decryption or parsing of the `EdgeeCookie` fails.
///
/// # Example
///
/// ```
/// let updated_cookie = decrypt_and_update("some_encrypted_cookie").unwrap();
/// println!("Updated EdgeeCookie: {:?}", updated_cookie);
/// ```
fn decrypt_and_update(
    encrypted_edgee_cookie: &str,
    payload: &Payload,
) -> Result<EdgeeCookie, &'static str> {
    let edgee_cookie_decrypted = decrypt(encrypted_edgee_cookie);
    if edgee_cookie_decrypted.is_err() {
        return Err("Failed to decrypt edgee_cookie");
    }

    // deserialize edgee_cookie
    let edgee_cookie_str = parse(edgee_cookie_decrypted?.as_bytes());
    if edgee_cookie_str.is_err() {
        return Err("Failed to parse edgee_cookie");
    }

    let mut edgee_cookie = edgee_cookie_str.unwrap();

    // if edgee_cookie_str.last_seen is not older than 30 minutes, update ls (last seen) and set cookie
    if Utc::now().signed_duration_since(edgee_cookie.ls) < SESSION_DURATION {
        edgee_cookie.ls = Utc::now();
    }

    // if lastSeen is older than 30 minutes, update ls (last seen) and session start (ss) and set cookie
    if Utc::now().signed_duration_since(edgee_cookie.ls) >= SESSION_DURATION {
        edgee_cookie.sc += 1;
        edgee_cookie.ps = Some(edgee_cookie.ss);
        let now = Utc::now();
        edgee_cookie.ls = now;
        edgee_cookie.ss = now;
    }
    edgee_cookie.set_screen_size(payload);
    edgee_cookie.set_client_hints(payload);

    Ok(edgee_cookie)
}

/// Initializes a new `EdgeeCookie`, encrypts it, and sets it in the response headers.
///
/// # Arguments
///
/// * `request` - A reference to the request headers.
/// * `response` - A mutable reference to the response headers where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Returns
///
/// * `EdgeeCookie` - The newly created and encrypted `EdgeeCookie`.
fn init_and_set_cookie(
    request: &RequestHandle,
    response: &mut Parts,
    payload: &Payload,
) -> EdgeeCookie {
    let mut edgee_cookie = EdgeeCookie::new();
    edgee_cookie.set_screen_size(payload);
    edgee_cookie.set_client_hints(payload);
    let edgee_cookie_str = serde_json::to_string(&edgee_cookie).unwrap();
    let edgee_cookie_encrypted = encrypt(&edgee_cookie_str).unwrap();
    set_cookie(
        &edgee_cookie_encrypted,
        response,
        request.get_host().as_str(),
    );
    edgee_cookie
}

/// Sets a cookie in the response headers.
///
/// # Arguments
///
/// * `value` - A string slice that holds the value of the cookie.
/// * `response` - A mutable reference to the response headers where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Panics
///
/// This function will panic if the `HeaderValue::from_str` function fails to convert the cookie string to a `HeaderValue`.
fn set_cookie(value: &str, response: &mut Parts, host: &str) {
    let secure = config::get().http.is_some() && config::get().http.as_ref().unwrap().force_https;
    let root_domain = get_root_domain(host);
    let cookie = Cookie::build((&config::get().compute.cookie_name, value))
        .domain(root_domain)
        .path("/")
        .http_only(false)
        .secure(secure)
        .same_site(SameSite::Lax)
        .expires(OffsetDateTime::now_utc() + StdDuration::from_secs(365 * 24 * 60 * 60));

    response.headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(cookie.to_string().as_str()).unwrap(),
    );
}

/// This function is used to extract the root domain from a given host.
///
/// # Arguments
///
/// * `host` - A string that holds the host from which the root domain will be extracted.
///
/// # Returns
///
/// * `String` - The root domain as a string. If the domain cannot be parsed, the original host string is returned.
///
/// # Errors
///
/// This function will return the original host string if the `addr::parse_domain_name` function fails to parse the domain.
fn get_root_domain(host: &str) -> String {
    // Attempt to parse the domain name from the host string
    let domain_result = addr::parse_domain_name(host);
    match domain_result {
        // If the domain name was successfully parsed
        Ok(domain) => {
            // Attempt to get the root of the domain
            // If the root domain is present, return it as a string
            if let Some(domain) = domain.root() {
                return domain.to_string();
            }
            // If the root domain is not present, return the original host string
            host.to_string()
        }
        // If the domain name could not be parsed, return the original host string
        Err(_) => host.to_string(),
    }
}

/// This function is used to parse a JSON string into an `EdgeeCookie` object.
///
/// # Type Parameters
///
/// * `T: Read` - The type of the input that implements the `Read` trait. This is typically a string that represents a JSON object.
///
/// # Arguments
///
/// * `clean_json: T` - The JSON string that will be parsed into an `EdgeeCookie` object.
///
/// # Returns
///
/// * `Result<EdgeeCookie, Error>` - The function returns a `Result` type. If the parsing is successful, it returns `Ok` containing the `EdgeeCookie` object. If the parsing fails, it returns `Err` containing the error information.
///
/// # Errors
///
/// This function will return an error if the JSON string cannot be parsed into an `EdgeeCookie` object.
fn parse<T: Read>(clean_json: T) -> Result<EdgeeCookie, Error> {
    serde_json::from_reader(clean_json)
}
