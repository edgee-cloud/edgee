mod config;
mod domains;
mod entrypoints;
mod logger;

#[tokio::main]
async fn main() {
    config::init();
    logger::init();
    let _ = entrypoints::start().await;
}
