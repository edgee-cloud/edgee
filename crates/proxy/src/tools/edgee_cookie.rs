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
use crate::proxy::compute::data_collection::payload::{Payload, User};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<String>, // consent
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
            c: None,
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

    fn set_consent(&mut self, payload: &Payload) {
        if let Some(consent) = payload
            .data_collection
            .as_ref()
            .and_then(|dc| dc.consent.as_ref())
        {
            self.c = Some(consent.to_string());
        }
    }
}

/// Retrieves an `EdgeeCookie` from the request headers or initializes a new one if it does not exist,
/// decrypts and updates it, and sets it in the response headers.
///
/// # Arguments
///
/// * `request` - A reference to the `RequestHandle` containing the request information.
/// * `response` - A mutable reference to the response `Parts` where the cookie will be set.
/// * `payload` - A reference to the `Payload` containing data collection information.
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

/// Checks if the edgee cookie exists in the request.
///
/// # Arguments
/// * `request` - A reference to the `RequestHandle` containing the request information.
///
/// # Returns
/// * `bool` - Returns true if the edgee cookie exists in the request, false otherwise.
pub fn has_cookie(request: &RequestHandle) -> bool {
    get_cookie(config::get().compute.cookie_name.as_str(), request).is_some()
}

/// Retrieves a cookie value from the request headers by name.
///
/// # Arguments
/// * `name` - The name of the cookie to retrieve
/// * `request` - A reference to the `RequestHandle` containing the request information
///
/// # Returns
/// * `Option<String>` - The cookie value if found, or None if not found
///
/// # Details
/// - Searches through all Cookie headers in the request
/// - Parses each cookie header into name-value pairs
/// - Returns the value if a cookie with the given name is found
/// - Falls back to checking query parameters if cookie not found in headers
pub fn get_cookie(name: &str, request: &RequestHandle) -> Option<String> {
    let all_cookies = request.get_headers().get_all(COOKIE);
    for cookie in all_cookies {
        let mut map = HashMap::new();
        for item in cookie.to_str().unwrap().split("; ") {
            if let Some((key, value)) = item.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        if map.contains_key(name) {
            return Some(map.get(name).unwrap().to_string());
        }
    }
    get_cookie_from_query_or_none(name, request)
}

/// Attempts to retrieve a cookie value from query parameters if not found in headers.
///
/// # Arguments
/// * `name` - The name of the cookie to retrieve
/// * `request` - A reference to the `RequestHandle` containing the request information
///
/// # Returns
/// * `Option<String>` - The cookie value if found in query params, or None if not found
///
/// # Details
/// - Only checks query params for POST requests to the third party SDK endpoint
/// - For edgee cookie, looks for 'e' param
/// - For edgee user cookie, looks for 'u' param
/// - Returns None for all other cases
fn get_cookie_from_query_or_none(name: &str, request: &RequestHandle) -> Option<String> {
    if request.get_path() == DATA_COLLECTION_ENDPOINT_FROM_THIRD_PARTY_SDK
        && request.get_method() == Method::POST
    {
        let mut param_name = "e";
        if name == format!("{}_u", config::get().compute.cookie_name) {
            param_name = "u";
        }
        let map: HashMap<String, String> = request
            .get_query()
            .split('&')
            .map(|s| s.split('=').collect::<Vec<&str>>())
            .filter(|v| v.len() == 2)
            .map(|v| (v[0].to_string(), v[1].to_string()))
            .collect();
        let e = map.get(param_name);
        e.map(|e| e.to_string())
    } else {
        None
    }
}

/// Retrieves an `EdgeeCookie` from the request headers, decrypts and updates it, and sets it in the response headers.
///
/// # Arguments
///
/// * `request` - A reference to the `RequestHandle` containing the request information.
/// * `response` - A mutable reference to the response `Parts` where the cookie will be set.
/// * `payload` - A reference to the `Payload` containing data collection information.
///
/// # Returns
///
/// * `Option<EdgeeCookie>` - An `Option` containing the `EdgeeCookie` if it exists and is successfully decrypted and updated,
///   or `None` if the cookie does not exist.
pub fn get(
    request: &RequestHandle,
    response: &mut Parts,
    payload: &Payload,
) -> Option<EdgeeCookie> {
    if let Some(value) = get_cookie(config::get().compute.cookie_name.as_str(), request) {
        let edgee_cookie_result = decrypt_and_update(value.as_str(), payload);
        if edgee_cookie_result.is_err() {
            return Some(init_and_set_cookie(request, response, payload));
        }
        let mut edgee_cookie = edgee_cookie_result.unwrap();
        edgee_cookie.set_screen_size(payload);
        edgee_cookie.set_client_hints(payload);
        edgee_cookie.set_consent(payload);
        let edgee_cookie_str = serde_json::to_string(&edgee_cookie).unwrap();
        let edgee_cookie_encrypted = encrypt(&edgee_cookie_str).unwrap();
        set_cookie(
            config::get().compute.cookie_name.as_str(),
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
/// ```ignore
/// let updated_cookie = decrypt_and_update("some_encrypted_cookie").unwrap();
/// println!("Updated EdgeeCookie: {:?}", updated_cookie);
/// ```
fn decrypt_and_update(value: &str, payload: &Payload) -> Result<EdgeeCookie, &'static str> {
    let decrypted = decrypt(value);
    if decrypted.is_err() {
        return Err("Failed to decrypt EdgeeCookie");
    }

    // deserialize EdgeeCookie
    let edgee_cookie_str = parse(decrypted?.as_bytes());
    if edgee_cookie_str.is_err() {
        return Err("Failed to parse EdgeeCookie");
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
    edgee_cookie.set_consent(payload);
    Ok(edgee_cookie)
}

/// Initializes a new `EdgeeCookie`, encrypts it, and sets it in the response headers.
///
/// # Arguments
///
/// * `request` - A reference to the `RequestHandle` containing the request information.
/// * `response` - A mutable reference to the response `Parts` where the cookie will be set.
/// * `payload` - A reference to the `Payload` containing data collection information.
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
    edgee_cookie.set_consent(payload);
    let edgee_cookie_str = serde_json::to_string(&edgee_cookie).unwrap();
    let edgee_cookie_encrypted = encrypt(&edgee_cookie_str).unwrap();
    set_cookie(
        config::get().compute.cookie_name.as_str(),
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
/// * `name` - A string slice that holds the name of the cookie.
/// * `value` - A string slice that holds the value of the cookie.
/// * `response` - A mutable reference to the response `Parts` where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Panics
///
/// This function will panic if the cookie string cannot be converted to a valid `HeaderValue`.
fn set_cookie(name: &str, value: &str, response: &mut Parts, host: &str) {
    let secure = config::get().http.is_some() && config::get().http.as_ref().unwrap().force_https;
    let cookie_domain = config::get()
        .compute
        .cookie_domain
        .clone()
        .unwrap_or_else(|| get_root_domain(host));
    let cookie = Cookie::build((name, value))
        .domain(cookie_domain)
        .path("/")
        .http_only(false)
        .secure(secure)
        .same_site(SameSite::Lax)
        .expires(OffsetDateTime::now_utc() + StdDuration::from_secs(365 * 24 * 60 * 60));

    response.headers.append(
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
pub fn get_root_domain(host: &str) -> String {
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

/// Parses a JSON payload into an EdgeeCookie struct.
///
/// # Arguments
/// * `clean_json` - Reader containing valid JSON data
///
/// # Returns
/// * `Ok(EdgeeCookie)` - Successfully parsed EdgeeCookie
/// * `Err(Error)` - JSON parsing error with details
///
/// # Errors
/// Returns error if:
/// - JSON is malformed
/// - JSON structure doesn't match EdgeeCookie schema
/// - IO error occurs while reading
fn parse<T: Read>(clean_json: T) -> Result<EdgeeCookie, Error> {
    serde_json::from_reader(clean_json)
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct EdgeeUserCookie {
    pub user_id: Option<String>,
    pub anonymous_id: Option<String>,
    pub properties: Option<HashMap<String, serde_json::Value>>,
}

/// Sets the user cookie in the response headers with encrypted user data.
///
/// # Arguments
/// * `request` - A reference to the `RequestHandle` containing the request information
/// * `response` - A mutable reference to the response `Parts` where the cookie will be set
/// * `user_data` - A reference to the `User` data to store in the cookie
///
/// # Details
/// - Creates an `EdgeeUserCookie` from the provided user data
/// - Serializes and encrypts the cookie data
/// - Sets the encrypted cookie in the response headers with the appropriate name
pub fn set_user_cookie(request: &RequestHandle, response: &mut Parts, user_data: &User) {
    let user_cookie = EdgeeUserCookie {
        user_id: user_data.user_id.clone(),
        anonymous_id: user_data.anonymous_id.clone(),
        properties: user_data.properties.clone(),
    };
    let user_cookie_str = serde_json::to_string(&user_cookie).unwrap();
    let encrypted_user_cookie = encrypt(&user_cookie_str).unwrap();
    let cookie_name = format!("{}_u", config::get().compute.cookie_name);
    set_cookie(
        &cookie_name,
        &encrypted_user_cookie,
        response,
        request.get_host().as_str(),
    );
}

/// Retrieves and decrypts the user cookie from the request headers.
///
/// # Arguments
/// * `request` - A reference to the `RequestHandle` containing the request information
///
/// # Returns
/// * `Option<EdgeeUserCookie>` - The decrypted user cookie if present and valid, None otherwise
///
/// # Details
/// - Looks for cookie with name "{cookie_name}_u" in request headers
/// - Attempts to decrypt and parse the cookie value if found
/// - Returns None if cookie is missing or invalid
pub fn get_user_cookie(request: &RequestHandle) -> Option<EdgeeUserCookie> {
    let cookie_name = format!("{}_u", config::get().compute.cookie_name);
    if let Some(value) = get_cookie(cookie_name.as_str(), request) {
        let cookie_result = decrypt_user_cookie(value.as_str());
        if cookie_result.is_err() {
            return None;
        }
        return Some(cookie_result.unwrap());
    }
    None
}

/// Decrypts and parses an encrypted user cookie string.
///
/// # Arguments
/// * `value` - The encrypted cookie string to decrypt
///
/// # Returns
/// * `Result<EdgeeUserCookie, &'static str>` - The decrypted and parsed cookie on success,
///   or an error message on failure
///
/// # Errors
/// Returns error if:
/// - Decryption fails
/// - JSON parsing fails after decryption
fn decrypt_user_cookie(value: &str) -> Result<EdgeeUserCookie, &'static str> {
    let decrypted = decrypt(value);
    if decrypted.is_err() {
        return Err("Failed to decrypt EdgeeUserCookie");
    }

    // deserialize EdgeeUserCookie
    let cookie_str: Result<EdgeeUserCookie, Error> = parse_user_cookie(decrypted?.as_bytes());
    if cookie_str.is_err() {
        return Err("Failed to parse EdgeeUserCookie");
    }

    Ok(cookie_str.unwrap())
}

/// Parses a JSON reader into an EdgeeUserCookie struct.
///
/// # Arguments
/// * `clean_json` - Reader containing valid JSON data
///
/// # Returns
/// * `Result<EdgeeUserCookie, Error>` - The parsed cookie struct on success,
///   or a serde_json Error on failure
///
/// # Errors
/// Returns error if:
/// - JSON is malformed
/// - JSON structure doesn't match EdgeeUserCookie schema
/// - IO error occurs while reading
fn parse_user_cookie<T: Read>(clean_json: T) -> Result<EdgeeUserCookie, Error> {
    serde_json::from_reader(clean_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::response;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn sample_user_data() -> User {
        User {
            user_id: Some("123".to_string()),
            anonymous_id: Some("456".to_string()),
            edgee_id: "456".to_string(),
            properties: Some(HashMap::from([
                ("prop1".to_string(), json!(true)),
                ("prop2".to_string(), json!(false)),
                ("prop3".to_string(), json!(10)),
                ("prop4".to_string(), json!("ok")),
            ])),
        }
    }

    #[test]
    fn test_get_root_domain() {
        assert_eq!(get_root_domain("example.com"), "example.com");
        assert_eq!(get_root_domain("test.example.com"), "example.com");
        assert_eq!(get_root_domain("sub.test.example.com"), "example.com");
        assert_eq!(get_root_domain("localhost"), "localhost");
    }

    #[test]
    fn test_set_user_cookie() {
        let request = RequestHandle::default();
        let response = response::Builder::new().status(200).body("").unwrap();
        let (mut parts, _body) = response.into_parts();

        set_user_cookie(&request, &mut parts, &sample_user_data());

        assert_eq!(parts.headers.len(), 1);
    }
}
