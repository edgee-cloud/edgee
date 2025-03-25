use clap::Parser;

mod commands;
mod components;
mod config;
mod logger;
mod telemetry;

#[derive(Debug, Parser)]
#[command(about, author, version)]
struct Options {
    #[command(subcommand)]
    command: commands::Command,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let options = Options::parse();

    if let Err(err) = crate::telemetry::setup() {
        tracing::debug!("Telemetry error: {err}");
    }

    telemetry::process_cli_command(options.command.run()).await
}
