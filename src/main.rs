use tracing::error;

mod config;
mod logger;
mod monitor;
mod proxy;
mod server;
mod tools;

#[tokio::main]
async fn main() {
    config::config::init();
    logger::logger::init();
    proxy::compute::data_collection::components::init();

    tokio::select! {
        Err(err) = monitor::start() => error!(?err, "Monitor failed"),
        Err(err) = server::start() => error!(?err, "Server failed to start"),
    }
}
