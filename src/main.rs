mod config {

    use serde::Deserialize;
    use tokio::sync::OnceCell;

    static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

    #[derive(Deserialize, Debug)]
    pub struct StaticConfiguration {
        pub http_port: u16,
        pub https_port: u16,
    }

    pub fn init() {
        let config_file = std::fs::read_to_string("edgee.toml").unwrap();
        let config: StaticConfiguration = toml::from_str(&config_file).unwrap();
        CONFIG.set(config).unwrap();
    }

    pub fn get() -> &'static StaticConfiguration {
        CONFIG.get().unwrap()
    }
}

fn main() {
    config::init();

    println!("http_port: {}", config::get().http_port);
    println!("https_port: {}", config::get().https_port);
}
