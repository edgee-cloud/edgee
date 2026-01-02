use clap::Parser;

mod commands;
mod components;
mod config;
mod logger;
mod telemetry;

use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(about, author, version)]
struct Options {
    #[command(subcommand)]
    command: commands::Command,
}

fn main() -> ExitCode {
    let _sentry = logger::init_sentry();
    // TODO: Replace with a better way to enable backtrace
    // Most likely by switch from anyhow to another error crate (like miette), since
    // there's no other way with anyhow
    unsafe {
        std::env::set_var("RUST_LIB_BACKTRACE", "1");
    }

    let options = Options::parse();

    if let Err(err) = crate::telemetry::setup() {
        tracing::debug!("Telemetry error: {err}");
    }

    let runtime = tokio::runtime::Runtime::new().expect("Could not create async runtime");
    if let Err(err) = runtime.block_on(telemetry::process_cli_command(options.command.run())) {
        logger::report_error(err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
