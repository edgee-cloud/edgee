pub mod auth;
mod upload;

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

    // if EDGEE_API_URL env var is set, redefine baseurl
    let baseurl = std::env::var("EDGEE_API_URL").unwrap_or(baseurl.to_string());

    // if EDGEE_API_TOKEN env var is set, redefine api_token
    let api_token = std::env::var("EDGEE_API_TOKEN").unwrap_or(api_token.to_string());

    let auth_value = format!("Bearer {api_token}");
    default_headers.insert(header::AUTHORIZATION, auth_value.try_into().unwrap());

    let client = reqwest::Client::builder()
        .default_headers(default_headers)
        .build()
        .unwrap();

    Client::new_with_client(&baseurl, client)
}

#[easy_ext::ext(ErrorExt)]
impl Error<types::ErrorResponse> {
    pub fn into_message(self) -> String {
        match self {
            Error::ErrorResponse(err) => err.error.message.clone(),
            _ => self.to_string(),
        }
    }
}

#[easy_ext::ext(ResultExt)]
impl<T> Result<T, Error<types::ErrorResponse>> {
    pub fn api_context(self, ctx: impl std::fmt::Display) -> anyhow::Result<T> {
        self.map_err(|err| anyhow::anyhow!("{ctx}: {}", err.into_message()))
    }

    pub fn api_with_context<F, C>(self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display,
    {
        self.map_err(|err| {
            let ctx = f();
            anyhow::anyhow!("{ctx}: {}", err.into_message())
        })
    }
}
