pub mod auth;

pub const PROD_BASEURL: &str = "https://api.edgee.app";

progenitor::generate_api! {
    spec = "openapi.json",
    interface = Builder,
    derives = [ schemars::JsonSchema ],
}

/// This crate's entry point
///
/// Use this function to build a client configured to interact with Edgee API using provided
/// credentials
#[bon::builder(
    start_fn = new,
    finish_fn = connect,
    on(String, into),
)]
pub fn connect(#[builder(default = PROD_BASEURL)] baseurl: &str, api_token: String) -> Client {
    use reqwest::header::{self, HeaderMap};

    let mut default_headers = HeaderMap::new();

    // if API_URL env var is set, redefine baseurl
    let baseurl = std::env::var("API_URL").unwrap_or(baseurl.to_string());

    // if API_TOKEN env var is set, redefine api_token
    let api_token = std::env::var("API_TOKEN").unwrap_or(api_token.to_string());

    let auth_value = format!("Bearer {api_token}");
    default_headers.insert(header::AUTHORIZATION, auth_value.try_into().unwrap());

    let client = reqwest::Client::builder()
        .default_headers(default_headers)
        .build()
        .unwrap();

    Client::new_with_client(&baseurl, client)
}
