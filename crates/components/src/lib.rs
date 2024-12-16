use std::sync::LazyLock;

pub mod auth;

const DEFAULT_API_ENDPOINT_BASE_URL: &str = "https://api.edgee.cloud";
pub static API_ENDPOINT_BASE_URL: LazyLock<String> = LazyLock::new(|| {
    use std::env;

    env::var("EDGEE_API_ENDPOINT_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_API_ENDPOINT_BASE_URL.to_owned())
});
