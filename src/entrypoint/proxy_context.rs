use http::{HeaderMap, HeaderValue, Uri};
use hyper::body::Incoming;
use hyper_rustls::ConfigBuilderExt;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use tracing::debug;
use crate::tools::real_ip::Realip;
use super::{incoming_context::IncomingContext, routing_context::RoutingContext};

pub struct ProxyContext<'a> {
    incoming_headers: HeaderMap,
    incoming_body: Incoming,
    incoming_uri: http::Uri,
    incoming_method: http::Method,
    routing_context: &'a RoutingContext,
}

impl<'a> ProxyContext<'a> {
    pub fn new(incoming_context: IncomingContext, routing_context: &'a RoutingContext) -> Self {
        const FORWARDED_FOR: &str = "x-forwarded-for";
        const FORWARDED_PROTO: &str = "x-forwarded-proto";
        const FORWARDED_HOST: &str = "x-forwarded-host";

        let mut incoming_headers = incoming_context.headers().clone();
        let incoming_uri = incoming_context.uri().clone();
        let incoming_method = incoming_context.method().clone();
        let incoming_host = incoming_context.host().clone();
        let incoming_body = incoming_context.incoming_body;

        // client ip
        let realip = Realip::new();
        let client_ip = realip.get_from_request(incoming_context.remote_addr, &incoming_headers);

        if let Some(forwarded_for) = incoming_headers.get_mut(FORWARDED_FOR) {
            let existing_value = forwarded_for.to_str().unwrap();
            let new_value = format!("{}, {}", existing_value, client_ip);
            *forwarded_for =
                HeaderValue::from_str(&new_value).expect("header value should be valid");
        } else {
            incoming_headers.insert(
                FORWARDED_FOR,
                HeaderValue::from_str(&client_ip).expect("header value should be valid"),
            );
        }

        if incoming_headers.get(FORWARDED_PROTO).is_none() {
            incoming_headers.insert(
                FORWARDED_PROTO,
                HeaderValue::from_str(if incoming_context.is_https {
                    "https"
                } else {
                    "http"
                })
                .expect("header value should be valid"),
            );
        }

        if incoming_headers.get(FORWARDED_HOST).is_none() {
            incoming_headers.insert(
                FORWARDED_HOST,
                HeaderValue::from_str(&incoming_host).expect("header value should be valid"),
            );
        }

        Self {
            incoming_headers,
            incoming_body,
            incoming_uri,
            incoming_method,
            routing_context,
        }
    }

    pub async fn response(self) -> anyhow::Result<http::Response<Incoming>> {
        if self.routing_context.backend.enable_ssl {
            self.forward_https_request().await
        } else {
            self.forward_http_request().await
        }
    }

    async fn forward_http_request(self) -> anyhow::Result<http::Response<Incoming>> {
        let backend = &self.routing_context.backend;
        let path = &self.routing_context.path;
        let uri: Uri = format!("http://{}{}", &backend.address, path)
            .parse()
            .expect("uri should be valid");

        debug!(origin=?self.incoming_uri,?uri, "Forwarding HTTP request");

        let mut req = http::Request::builder()
            .uri(uri)
            .method(&self.incoming_method);
        let headers = req.headers_mut().expect("request should have headers");
        for (name, value) in self.incoming_headers.iter() {
            headers.insert(name, value.to_owned());
        }

        headers.insert(
            "host",
            HeaderValue::from_str(&backend.address).expect("host should be valid"),
        );

        let req = req.body(self.incoming_body).expect("request to be built");
        let client = Client::builder(TokioExecutor::new()).build(HttpConnector::new());
        client
            .request(req)
            .await
            .map_err(|err| anyhow::Error::new(err))
    }

    async fn forward_https_request(self) -> anyhow::Result<http::Response<Incoming>> {
        let backend = &self.routing_context.backend;
        let path = &self.routing_context.path;
        let uri: Uri = format!("https://{}{}", &backend.address, path)
            .parse()
            .expect("uri should be valid");

        let mut req = http::Request::builder()
            .uri(uri)
            .method(&self.incoming_method);
        let headers = req.headers_mut().expect("request should have headers");
        for (name, value) in self.incoming_headers.iter() {
            headers.insert(name, value.to_owned());
        }

        headers.insert(
            "host",
            HeaderValue::from_str(&backend.address).expect("host should be valid"),
        );

        let req = req.body(self.incoming_body).expect("request to be built");
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
}
