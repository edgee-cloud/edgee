use std::collections::HashMap;

pub(crate) fn init(config: HashMap<String, toml::Value>) {
    let provider: toml::Value = config.get("file").unwrap().clone();
    tracing::debug!("provider: {:?}", provider);
}
