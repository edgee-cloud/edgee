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

fn main() {
    let _sentry = logger::init_sentry();
    std::env::set_var("RUST_LIB_BACKTRACE", "1");

    let options = Options::parse();

    if let Err(err) = crate::telemetry::setup() {
        tracing::debug!("Telemetry error: {err}");
    }

    let runtime = tokio::runtime::Runtime::new().expect("Could not create async runtime");
    if let Err(err) = runtime.block_on(telemetry::process_cli_command(options.command.run())) {
        sentry_anyhow::capture_anyhow(&err);

        for (idx, err) in err.chain().enumerate() {
            let spacing = if idx > 0 { "  " } else { "" };
            tracing::error!("{spacing}{err}");
        }
    }
}
