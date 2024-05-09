#[derive(Clone)]
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

#[derive(Clone)]
pub struct Routing {
    pub path: String,
    pub rank: i32,
    pub backend_name: String,
    pub regex: bool,
    pub rewritepath_regex: String,
    pub rewritepath_replace: String,
}

#[derive(Clone)]
pub struct Backend {
    pub name: String,
    pub location: String,
    pub override_host: String,
    pub default: bool,
}

#[derive(Clone)]
pub struct Provider {
    endpoints: Vec<Endpoint>,
}

impl Provider {
    pub fn get(&self, hostname: String) -> Option<&Endpoint> {
        for endpoint in &self.endpoints {
            if endpoint.hostname == hostname {
                return Some(&endpoint);
            }
        }
        return None;
    }
}

pub fn load() -> Provider {
    let recoeur = Endpoint {
        id: String::from("recoeur"),
        hostname: String::from("recoeur.edgee.cloud"),
        backend: vec![Backend {
            name: String::from("home"),
            location: String::from("localhost:9000"),
            override_host: String::from("recoeur.github.io"),
            default: true,
        }],
        routing: vec![],
    };

    Provider {
        endpoints: vec![recoeur],
    }
}
