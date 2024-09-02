use crate::config;
use crate::tools::crypto::{decrypt, encrypt};
use cookie::time::OffsetDateTime;
use cookie::{Cookie, SameSite};
use std::collections::HashMap;
use std::time::Duration as StdDuration;
use std::io::Read;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use http::header::{COOKIE, SET_COOKIE};
use http::HeaderValue;
use serde::{Deserialize, Serialize};
use serde_json::Error;

static SESSION_DURATION: Duration = Duration::minutes(30);

#[derive(Serialize, Deserialize, Debug)]
pub struct EdgeeCookie {
    pub id: Uuid, // v4 uuid
    pub fs: DateTime<Utc>, //first seen
    pub ls: DateTime<Utc>, // last seen
    pub ss: DateTime<Utc>, // session start (used to create session_id as well)
    #[serde(skip_serializing)]
    pub ps: Option<DateTime<Utc>>, // previous session
    pub sc: u32, // session count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sz: Option<String>, // screen size
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
        }
    }
}


/// Retrieves an `EdgeeCookie` from the request headers or initializes a new one if it does not exist,
/// decrypts and updates it, and sets it in the response headers.
///
/// # Arguments
///
/// * `req_headers` - A reference to the request headers.
/// * `res_headers` - A mutable reference to the response headers where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Returns
///
/// * `EdgeeCookie` - The `EdgeeCookie` that was retrieved or newly created.
pub fn get_or_set(req_headers: &http::HeaderMap, res_headers: &mut http::HeaderMap, host: &str) -> EdgeeCookie {
    let edgee_cookie = get(req_headers, res_headers, host);
    if edgee_cookie.is_none() {
        return init_and_set_cookie(res_headers, host);
    }
    edgee_cookie.unwrap()
}

/// Retrieves an `EdgeeCookie` from the request headers, decrypts and updates it, and sets it in the response headers.
///
/// # Arguments
///
/// * `req_headers` - A reference to the request headers.
/// * `res_headers` - A mutable reference to the response headers where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Returns
///
/// * `Option<EdgeeCookie>` - An `Option` containing the `EdgeeCookie` if it exists and is successfully decrypted and updated, or `None` if the cookie does not exist or decryption fails.
pub fn get(req_headers: &http::HeaderMap, res_headers: &mut http::HeaderMap, host: &str) -> Option<EdgeeCookie> {
    let edgee_cookie = match req_headers.get(COOKIE) {
        Some(cookie) => {
            // Put cookies value into a map
            let mut map = HashMap::new();
            for item in cookie.to_str().unwrap().split("; ") {
                let parts: Vec<&str> = item.split('=').collect();
                map.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
            }

            if let Some(value) = map.get(&config::get().security.cookie_name) {
                let encrypted_edgee_cookie = value.to_string();
                let edgee_cookie_result = decrypt_and_update(&encrypted_edgee_cookie);
                if edgee_cookie_result.is_err() {
                    return Some(init_and_set_cookie(res_headers, host));
                }
                let edgee_cookie = edgee_cookie_result.unwrap();

                let edgee_cookie_str = serde_json::to_string(&edgee_cookie).unwrap();
                let edgee_cookie_encrypted = encrypt(&edgee_cookie_str).unwrap();
                set_cookie(&edgee_cookie_encrypted, res_headers, host);

                return Some(edgee_cookie);
            }
            None
        }
        None => None,
    };
    return edgee_cookie;
}

/// Decrypts an encrypted `EdgeeCookie`, updates its fields based on the current time, and returns the updated `EdgeeCookie`.
///
/// # Arguments
///
/// * `encrypted_edgee_cookie` - A string slice that holds the encrypted `EdgeeCookie`.
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
pub fn decrypt_and_update(encrypted_edgee_cookie: &str) -> Result<EdgeeCookie, &'static str> {
    let edgee_cookie_decrypted = decrypt(&encrypted_edgee_cookie);
    if edgee_cookie_decrypted.is_err() {
        return Err("Failed to decrypt edgee_cookie");
    }

    // deserialize edgee_cookie
    let edgee_cookie_str = parse(edgee_cookie_decrypted.unwrap().as_bytes());
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
        edgee_cookie.sc = edgee_cookie.sc + 1;
        edgee_cookie.ps = Some(edgee_cookie.ss);
        let now = Utc::now();
        edgee_cookie.ls = now;
        edgee_cookie.ss = now;
    }

    Ok(edgee_cookie)
}


/// Initializes a new `EdgeeCookie`, encrypts it, and sets it in the response headers.
///
/// # Arguments
///
/// * `req_headers` - A reference to the request headers.
/// * `res_headers` - A mutable reference to the response headers where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Returns
///
/// * `EdgeeCookie` - The newly created and encrypted `EdgeeCookie`.
fn init_and_set_cookie(res_headers: &mut http::HeaderMap, host: &str) -> EdgeeCookie {
    let edgee_cookie = EdgeeCookie::new();
    let edgee_cookie_str = serde_json::to_string(&edgee_cookie).unwrap();
    let edgee_cookie_encrypted = encrypt(&edgee_cookie_str).unwrap();
    set_cookie(&edgee_cookie_encrypted, res_headers, host);
    edgee_cookie
}


/// Sets a cookie in the response headers.
///
/// # Arguments
///
/// * `value` - A string slice that holds the value of the cookie.
/// * `res_headers` - A mutable reference to the response headers where the cookie will be set.
/// * `host` - A string slice that holds the host for which the cookie is set.
///
/// # Panics
///
/// This function will panic if the `HeaderValue::from_str` function fails to convert the cookie string to a `HeaderValue`.
fn set_cookie(value: &str, res_headers: &mut http::HeaderMap, host: &str) {
    let root_domain = get_root_domain(host);
    let cookie = Cookie::build((&config::get().security.cookie_name, value))
        .domain(root_domain)
        .path("/")
        .http_only(false)
        .secure(config::get().http.force_https)
        .same_site(SameSite::Lax)
        .expires(OffsetDateTime::now_utc() + StdDuration::from_secs(365 * 24 * 60 * 60));

    res_headers.insert(SET_COOKIE, HeaderValue::from_str(cookie.to_string().as_str()).unwrap());
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
            let domain = domain.root();
            // If the root domain is present, return it as a string
            if domain.is_some() {
                return domain.unwrap().to_string();
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
    match serde_json::from_reader(clean_json) {
        Ok(edgee_cookie) => Ok(edgee_cookie),
        Err(e) => Err(e),
    }
}
