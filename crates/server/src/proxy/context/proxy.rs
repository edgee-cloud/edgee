use http::{HeaderMap, HeaderValue, Uri};
use hyper::body::Incoming;
use hyper_rustls::ConfigBuilderExt;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};

use crate::config;

use super::body::ProxyBody;
use super::incoming::IncomingContext;
use super::routing::RoutingContext;

pub type Response = http::Response<ProxyBody>;

pub struct ProxyContext<'a> {
    incoming_headers: HeaderMap,
    incoming_body: Incoming,
    incoming_method: http::Method,
    routing_context: &'a RoutingContext,
}

impl<'a> ProxyContext<'a> {
    pub fn new(incoming_context: IncomingContext, routing_context: &'a RoutingContext) -> Self {
        const FORWARDED_FOR: &str = "x-forwarded-for";
        const FORWARDED_PROTO: &str = "x-forwarded-proto";
        const FORWARDED_HOST: &str = "x-forwarded-host";

        let mut incoming_headers = incoming_context.request.get_headers().clone();
        let incoming_method = incoming_context.request.get_method().clone();
        let incoming_body = incoming_context.body;
        let client_ip = incoming_context.request.get_client_ip();

        if let Some(forwarded_for) = incoming_headers.get_mut(FORWARDED_FOR) {
            let existing_value = forwarded_for.to_str().unwrap();
            let new_value = format!("{existing_value}, {client_ip}");
            *forwarded_for =
                HeaderValue::from_str(&new_value).expect("header value should be valid");
        } else {
            incoming_headers.insert(
                FORWARDED_FOR,
                HeaderValue::from_str(client_ip).expect("header value should be valid"),
            );
        }

        if incoming_headers.get(FORWARDED_PROTO).is_none() {
            incoming_headers.insert(
                FORWARDED_PROTO,
                HeaderValue::from_str(incoming_context.request.get_proto())
                    .expect("header value should be valid"),
            );
        }

        if incoming_headers.get(FORWARDED_HOST).is_none() {
            incoming_headers.insert(
                FORWARDED_HOST,
                HeaderValue::from_str(incoming_context.request.get_host())
                    .expect("header value should be valid"),
            );
        }

        // rebuild all the cookies
        let cookies = incoming_headers.get_all("cookie");
        let mut filtered_cookies = Vec::new();
        for cookie in cookies {
            let cookie_str = cookie.to_str().unwrap();
            let cookie_parts: Vec<&str> = cookie_str.split(";").map(|c| c.trim()).collect();
            for cookie_part in cookie_parts {
                if let Some((key, _value)) = cookie_part.split_once('=') {
                    let name = key.trim();
                    if name != config::get().compute.cookie_name
                        && name != format!("{}_u", config::get().compute.cookie_name)
                    {
                        filtered_cookies.push(cookie_part.to_string());
                    }
                }
            }
        }
        if !filtered_cookies.is_empty() {
            let new_cookies = filtered_cookies.join("; ");
            incoming_headers.insert(
                "cookie",
                HeaderValue::from_str(&new_cookies).expect("header value should be valid"),
            );
        } else {
            incoming_headers.remove("cookie");
        }

        Self {
            incoming_headers,
            incoming_body,
            incoming_method,
            routing_context,
        }
    }

    pub async fn forward_request(self) -> anyhow::Result<Response> {
        use tower::{Service, ServiceBuilder, ServiceExt};

        use crate::config::BackendConfiguration;

        let BackendConfiguration {
            enable_ssl,
            compress,
            ..
        } = self.routing_context.backend;

        let client_builder = Client::builder(TokioExecutor::new());

        let client = if enable_ssl {
            let client_config = rustls::ClientConfig::builder()
                .with_native_roots()?
                .with_no_client_auth();
            let connector = hyper_rustls::HttpsConnectorBuilder::new()
                .with_tls_config(client_config)
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build();

            client_builder.build(connector).boxed()
        } else {
            client_builder.build_http().boxed()
        };

        let service_builder = ServiceBuilder::new();
        let mut client = service_builder.service(client);
        let client = client.ready().await?;

        let req = self.build_request();
        let (parts, body) = client.call(req).await?.into_parts();
        let body = if !compress {
            ProxyBody::uncompressed(body).await?
        } else {
            ProxyBody::compressed(&parts, body).await?
        };

        Ok(http::Response::from_parts(parts, body))
    }

    fn build_request(self) -> http::Request<Incoming> {
        let backend = &self.routing_context.backend;

        let path = &self.routing_context.path;
        let proto = if backend.enable_ssl { "https" } else { "http" };
        let uri: Uri = format!("{proto}://{}{path}", &backend.address)
            .parse()
            .expect("uri should be valid");

        let mut req = http::Request::builder()
            .uri(uri)
            .method(&self.incoming_method);

        let headers = req.headers_mut().expect("request should have headers");
        headers.extend(self.incoming_headers);

        headers.insert(
            "host",
            HeaderValue::from_str(&backend.address).expect("host should be valid"),
        );

        if !backend.compress {
            headers.remove("accept-encoding");
        }

        req.body(self.incoming_body).expect("request to be built")
    }
}
