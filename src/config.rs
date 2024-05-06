use std::collections::HashMap;

use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(about)]
pub(crate) struct Args {
    #[arg(short, long)]
    pub config_file: String, // path to the configuration file
}

#[derive(Deserialize)]
pub(crate) struct Config {
    pub http_port: u16,
    pub https_port: u16,
    pub log_severity: String,
    pub providers: HashMap<String, toml::Value>,
}

pub(crate) fn parse() -> Config {
    let arg = Args::parse();
    let config_file = std::fs::read_to_string(&arg.config_file).unwrap();
    let config: Config = toml::from_str(&config_file).unwrap();
    config
}
