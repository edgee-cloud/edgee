use std::path::PathBuf;
use clap::{Parser, Subcommand};

pub fn get_cli() -> Cli {
    Cli::parse()
}

#[derive(Parser)]
#[command(version, about, long_about = None)] // Read from `Cargo.toml`
pub struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Starts the server
    Start {
        /// the port to listen on
        #[arg(short, long, default_value_t = 8080, value_name = "PORT")]
        port: u16,

        /// The file that contains the configuration to apply.
        #[arg(short, long, value_name = "FILE")]
        file: PathBuf,
    },
    /// Stops the server
    Stop {
        /// The port to stop the server on
        #[arg(short, long, default_value_t = 8080, value_name = "PORT")]
        port: u16,
    },

    /// Pull an edgee integration
    Pull {
        /// The name of the integration to pull
        #[arg(short, long, value_name = "NAME[:TAG]")]
        name: String,
    },
}
