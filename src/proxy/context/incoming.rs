use crate::tools::real_ip::Realip;
use anyhow::Context;
use http::header::COOKIE;
use http::{header::HOST, uri::PathAndQuery, HeaderMap};
use hyper::body::Incoming;
use std::{net::SocketAddr, str::FromStr};

pub struct IncomingContext {
    pub body: Incoming,
    pub request: RequestHandle,
}

impl IncomingContext {
    pub fn new(request: http::Request<Incoming>, remote_addr: SocketAddr, proto: &str) -> Self {
        let (head, body) = request.into_parts();

        let mut req = RequestHandle::default();

        req.host = match (head.headers.get(HOST), head.uri.host()) {
            (None, Some(value)) => Some(String::from(value)),
            (Some(value), _) => Some(value.to_str().unwrap().to_string()),
            (None, None) => None,
        }
        .and_then(|host| host.split(':').next().map(|s| s.to_string()))
        .context("extracting hostname from request")
        .unwrap();

        let root_path = PathAndQuery::from_str("/").expect("'/' should be a valid path");
        let path = head.uri.path_and_query().unwrap_or(&root_path).to_owned();

        req.path_and_query = path.clone();
        req.path = path.path().to_string();
        req.query = path.query().unwrap_or("").to_string();
        req.method = head.method.clone();
        req.headers = head.headers.clone();

        // is_https
        let mut is_https = proto == "https";
        // check if the x-forwarded-proto header is set
        if let Some(forwarded_proto) = head.headers.get("x-forwarded-proto") {
            if let Ok(value) = forwarded_proto.to_str() {
                if value == "https" {
                    is_https = true;
                }
            }
        }

        req.is_https = is_https;
        if is_https {
            req.proto = "https".to_string();
        } else {
            req.proto = "http".to_string();
        }

        // debug mode
        let all_cookies = head.headers.get_all(COOKIE);
        let is_debug_mode = all_cookies
            .iter()
            .filter_map(|cookie| cookie.to_str().ok())
            .any(|cookie| cookie.contains("_edgeedebug=true"));

        req.is_debug_mode = is_debug_mode;

        // client ip
        let client_ip = Realip::new().get_from_request(&remote_addr, &head.headers);
        req.client_ip = client_ip;

        Self { body, request: req }
    }

    pub fn get_request(&self) -> &RequestHandle {
        &self.request
    }
}

#[derive(Debug, Clone)]
pub struct RequestHandle {
    headers: HeaderMap,
    method: http::Method,
    is_https: bool,
    proto: String,
    is_debug_mode: bool,
    client_ip: String,
    host: String,
    path: String,
    query: String,
    path_and_query: PathAndQuery,
}

impl Default for RequestHandle {
    fn default() -> Self {
        Self {
            headers: HeaderMap::new(),
            method: http::Method::GET,
            is_https: false,
            proto: String::new(),
            is_debug_mode: false,
            client_ip: String::new(),
            host: String::new(),
            path: "".to_string(),
            query: "".to_string(),
            path_and_query: PathAndQuery::from_str("/").unwrap(),
        }
    }
}

impl RequestHandle {
    pub fn get_headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn get_header(&self, key: impl http::header::AsHeaderName) -> Option<String> {
        self.headers
            .get(key)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
    }

    pub fn get_method(&self) -> &http::Method {
        &self.method
    }

    pub fn is_https(&self) -> bool {
        self.is_https
    }

    pub fn get_proto(&self) -> &String {
        &self.proto
    }

    pub fn is_debug_mode(&self) -> bool {
        self.is_debug_mode
    }

    pub fn get_client_ip(&self) -> &String {
        &self.client_ip
    }

    pub fn get_host(&self) -> &String {
        &self.host
    }
    pub fn get_path(&self) -> &String {
        &self.path
    }

    pub fn get_query(&self) -> &String {
        &self.query
    }

    pub fn get_path_and_query(&self) -> &PathAndQuery {
        &self.path_and_query
    }

    pub fn get_content_type(&self) -> String {
        self.get_header(http::header::CONTENT_TYPE)
            .unwrap_or_else(|| "".to_string())
    }
}
