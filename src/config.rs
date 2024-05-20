use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(about)]
pub struct Args {
    #[arg(short, long)]
    pub config_file: String, // path to the configuration file
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub http_port: u16,
    pub https_port: u16,
    pub force_https: bool,
    pub log: LogConfig,
}

#[derive(Deserialize, Clone)]
pub struct LogConfig {
    pub level: String,
}

pub fn parse() -> Config {
    let arg = Args::parse();
    let config_file = std::fs::read_to_string(&arg.config_file).unwrap();
    let config: Config = toml::from_str(&config_file).unwrap();
    config
}
