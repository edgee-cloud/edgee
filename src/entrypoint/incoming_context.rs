use std::{net::SocketAddr, str::FromStr};

use anyhow::Context;
use http::{header::HOST, request::Parts, uri::PathAndQuery, HeaderMap};
use hyper::body::Incoming;

pub struct IncomingContext {
    pub incoming_parts: Parts,
    pub incoming_body: Incoming,
    pub remote_addr: SocketAddr,
    pub is_https: bool,
    pub is_debug_mode: bool,
    host: String,
    path: PathAndQuery,
}

impl IncomingContext {
    pub fn new(request: http::Request<Incoming>, remote_addr: SocketAddr, proto: &str) -> Self {
        let (incoming_parts, incoming_body) = request.into_parts();

        let host = match (incoming_parts.headers.get(HOST), incoming_parts.uri.host()) {
            (None, Some(value)) => Some(String::from(value)),
            (Some(value), _) => Some(value.to_str().unwrap().to_string()),
            (None, None) => None,
        }
        .and_then(|host| host.split(':').next().map(|s| s.to_string()))
        .context("extracting hostname from request")
        .unwrap();

        let root_path = PathAndQuery::from_str("/").expect("'/' should be a valid path");
        let path = incoming_parts
            .uri
            .path_and_query()
            .unwrap_or(&root_path)
            .to_owned();

        // is_https
        let mut is_https = proto == "https";
        // check if the x-forwarded-proto header is set
        if let Some(forwarded_proto) = incoming_parts.headers.get("x-forwarded-proto") {
            if let Ok(value) = forwarded_proto.to_str() {
                if value == "https" {
                    is_https = true;
                }
            }
        }

        // debug mode
        let is_debug_mode = match incoming_parts.headers.get("edgee-debug") {
            Some(_) => true,
            None => false,
        };

        Self {
            incoming_parts,
            incoming_body,
            remote_addr,
            is_https,
            is_debug_mode,
            host,
            path,
        }
    }

    pub fn method(&self) -> &http::Method {
        &self.incoming_parts.method
    }

    pub fn path(&self) -> &http::uri::PathAndQuery {
        &self.path
    }

    pub fn uri(&self) -> &http::Uri {
        &self.incoming_parts.uri
    }

    pub fn host(&self) -> &String {
        &self.host
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.incoming_parts.headers
    }

    pub fn header(&self, key: impl http::header::AsHeaderName) -> Option<String> {
        self.incoming_parts
            .headers
            .get(key)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
    }
}
