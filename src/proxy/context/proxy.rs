use http::{HeaderMap, HeaderValue, Uri};
use hyper::body::Incoming;
use hyper_rustls::ConfigBuilderExt;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};

use super::{incoming::IncomingContext, routing::RoutingContext};

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
        let mut str_cookies = String::new();
        for cookie in cookies {
            let cookie_str = cookie.to_str().unwrap();
            let cookie_parts: Vec<&str> = cookie_str.split("; ").collect();
            for cookie_part in cookie_parts {
                let parts: Vec<&str> = cookie_part.split('=').collect();
                let name = parts[0].trim();
                let value = parts[1].trim();
                str_cookies.push_str(&format!("{}={}; ", name, value));
            }
        }
        if !str_cookies.is_empty() {
            incoming_headers.insert(
                "cookie",
                HeaderValue::from_str(&str_cookies).expect("header value should be valid"),
            );
        }

        Self {
            incoming_headers,
            incoming_body,
            incoming_method,
            routing_context,
        }
    }

    pub async fn forward_request(self) -> anyhow::Result<http::Response<Incoming>> {
        use tower::{Service, ServiceBuilder, ServiceExt};

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
        for (name, value) in self.incoming_headers.iter() {
            headers.insert(name, value.to_owned());
        }

        headers.insert(
            "host",
            HeaderValue::from_str(&backend.address).expect("host should be valid"),
        );

        let req = req.body(self.incoming_body).expect("request to be built");

        let service_builder = ServiceBuilder::new();

        let client_builder = Client::builder(TokioExecutor::new());

        let mut client = if backend.enable_ssl {
            let client_config = rustls::ClientConfig::builder()
                .with_native_roots()?
                .with_no_client_auth();
            let connector = hyper_rustls::HttpsConnectorBuilder::new()
                .with_tls_config(client_config)
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build();

            service_builder
                .service(client_builder.build(connector))
                .boxed()
        } else {
            let connector = HttpConnector::new();

            service_builder
                .service(client_builder.build(connector))
                .boxed()
        };

        let client = client.ready().await?;

        client.call(req).await.map_err(Into::into)
    }
}
