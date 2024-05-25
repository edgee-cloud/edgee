use serde::Deserialize;

use crate::config;

#[derive(Deserialize)]
pub struct Endpoint {
    pub id: String,
    pub hostname: String,
    pub routing: Vec<Routing>,
    pub backend: Vec<Backend>,
}

impl Endpoint {
    pub fn get_backend(&self, _route: String) -> Option<&Backend> {
        self.backend.iter().find(|b| b.default)
    }
}

#[derive(Deserialize)]
pub struct Routing {
    pub path: String,
    pub rank: i32,
    pub backend_name: String,
    pub regex: bool,
    pub rewritepath_regex: String,
    pub rewritepath_replace: String,
}

#[derive(Deserialize)]
pub struct Backend {
    pub name: String,
    pub location: String,
    pub override_host: String,
    pub default: bool,
}

#[derive(Deserialize)]
pub struct Provider {
    endpoints: Vec<Endpoint>,
}

impl Provider {
    pub fn get(&self, hostname: String) -> Option<&Endpoint> {
        self.endpoints
            .iter()
            .find(|&endpoint| endpoint.hostname == hostname)
    }
}

// TODO: Remove unwrap
pub fn load(cfg: &config::ProviderConfig) -> Provider {
    let file = std::fs::read_to_string(&cfg.file.filename).unwrap();
    let provider: Provider = toml::from_str(&file).unwrap();
    provider
}
