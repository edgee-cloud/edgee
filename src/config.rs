use serde::Deserialize;
use tokio::sync::OnceCell;

static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

#[derive(Deserialize, Debug, Clone)]
pub struct StaticConfiguration {
    pub log: LogConfiguration,
    pub entrypoints: Vec<EntryPointConfiguration>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogConfiguration {
    pub level: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct EntryPointConfiguration {
    pub name: String,
    pub bind: String,
    pub tls: bool,
    pub domains: Vec<DomainConfiguration>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DomainConfiguration {
    pub host: String,
}

pub fn init() {
    let config_file = std::fs::read_to_string("edgee.toml").unwrap();
    let config: StaticConfiguration = toml::from_str(&config_file).unwrap();
    CONFIG.set(config).unwrap();
}

pub fn get() -> &'static StaticConfiguration {
    CONFIG.get().unwrap()
}
