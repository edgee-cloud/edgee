progenitor::generate_api! {
    spec = "openapi.json",
    interface = Builder,
    derives = [ schemars::JsonSchema ],
}

pub const PROD_BASEURL: &str = "https://api.edgee.app";

impl Client {
    pub fn new_prod() -> Self {
        Self::new(PROD_BASEURL)
    }
}
