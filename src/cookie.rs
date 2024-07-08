use crate::config;
use crate::crypto::{decrypt, encrypt};
use chrono::{DateTime, Duration, Utc};
use cookie::time::OffsetDateTime;
use cookie::{Cookie, SameSite};
use http::header::COOKIE;
use http::HeaderValue;
use serde::{Deserialize, Serialize};
use serde_json::Error;
use std::collections::HashMap;
use std::io::Read;
use std::time::Duration as StdDuration;
use uuid::Uuid;

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

pub fn get_or_create(
    req_headers: &http::HeaderMap,
    res_headers: &mut http::HeaderMap,
    host: &str,
    is_https: bool,
) -> EdgeeCookie {
    let cookies = req_headers
        .get(COOKIE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let (cookie_str, edgee_cookie) = get(host, is_https, cookies);
    res_headers.insert(COOKIE, HeaderValue::from_str(&cookie_str).unwrap());
    return edgee_cookie;
}

pub fn get(host: &str, is_https: bool, cookies: &str) -> (String, EdgeeCookie) {
    let mut hashmap = HashMap::new();
    for item in cookies.split("; ") {
        let parts: Vec<&str> = item.split('=').collect();
        hashmap.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
    }

    hashmap
        .get(&config::get().security.cookie_name)
        .and_then(|value| update(value))
        .map(|cookie| build_cookie(Some(cookie), &host, is_https))
        .unwrap_or_else(|| build_cookie(None, &host, is_https))
}

pub fn update(encrypted_edgee_cookie: &str) -> Option<EdgeeCookie> {
    decrypt(&encrypted_edgee_cookie)
        .and_then(|decrypted| parse(decrypted.as_bytes()).ok())
        .map(|mut edgee_cookie| {
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

            edgee_cookie
        })
}

fn build_cookie(cookie: Option<EdgeeCookie>, host: &str, is_https: bool) -> (String, EdgeeCookie) {
    let edgee_cookie = cookie.unwrap_or(EdgeeCookie::new());
    let edgee_cookie_str = serde_json::to_string(&edgee_cookie).unwrap();
    let edgee_cookie_encrypted = encrypt(&edgee_cookie_str);
    let cookie_name = &config::get().security.cookie_name;
    let cookie_string = Cookie::build((cookie_name, edgee_cookie_encrypted))
        .domain(get_root_domain(host.to_string()))
        .path("/")
        .http_only(false)
        .secure(is_https)
        .same_site(SameSite::Lax)
        .expires(OffsetDateTime::now_utc() + StdDuration::from_secs(365 * 24 * 60 * 60))
        .to_string();
    (cookie_string, edgee_cookie)
}

fn get_root_domain(host: String) -> String {
    match addr::parse_domain_name(&host)
        .ok()
        .and_then(|domain| domain.root())
    {
        Some(root) => root.to_string(),
        None => host,
    }
}

fn parse<T: Read>(clean_json: T) -> Result<EdgeeCookie, Error> {
    match serde_json::from_reader(clean_json) {
        Ok(edgee_cookie) => Ok(edgee_cookie),
        Err(e) => Err(e),
    }
}
