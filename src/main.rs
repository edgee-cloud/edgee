#![deny(warnings)]

mod config;

use bytes::Bytes;
use clap::{Parser, Subcommand};
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::client::conn::http1::Builder;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::{TcpListener, TcpStream};

#[derive(Parser)]
#[command(version, about, long_about = None)] // Read from `Cargo.toml`
struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Start { port, file } => {

            // configuration
            let conf = match config::configure(file) {
                Ok(conf) => conf,
                Err(_) => unreachable!(),
            };

            println!("File: {:?}", conf);

            let addr = SocketAddr::from(([127, 0, 0, 1], *port));

            let listener = TcpListener::bind(addr).await?;
            println!("Listening on http://{}", addr);

            // We start a loop to continuously accept incoming connections
            loop {
                let (stream, _) = listener.accept().await?;
                let io = TokioIo::new(stream);

                tokio::task::spawn(async move {
                    if let Err(err) = http1::Builder::new()
                        .preserve_header_case(true)
                        .title_case_headers(true)
                        .serve_connection(io, service_fn(proxy))
                        .with_upgrades()
                        .await
                    {
                        println!("Failed to serve connection: {:?}", err);
                    }
                });
            }
        }
        _ => unreachable!(),
    }
}

async fn proxy(req: Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    println!("req: {:?} {:?}", req.method(), req.uri());

    let host = "localhost";
    let port = req.uri().port_u16().unwrap_or(3000);

    let stream = TcpStream::connect((host, port)).await.unwrap();
    let io = TokioIo::new(stream);

    let (mut sender, conn) = Builder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .handshake(io)
        .await?;
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    let resp = sender.send_request(req).await?;
    Ok(resp.map(|b| b.boxed()))
}
