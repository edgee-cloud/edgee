use tracing::error;

mod config;
mod data_collection;
mod destinations;
mod entrypoint;
mod html;
mod logger;
mod monitor;
mod tools;
mod edge;

#[tokio::main]
async fn main() {
    config::init();
    logger::init();
    destinations::init();

    // FIXME: Add gracefull shutdown
    tokio::select! {
        Err(err) = monitor::start() => error!(?err, "Monitor failed"),
        Err(err) = entrypoint::start() => error!(?err, "Entrypoint failed"),
    }
}
