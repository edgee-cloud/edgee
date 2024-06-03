use tracing::error;

mod config;
mod entrypoint;
mod logger;
mod monitor;

#[tokio::main]
async fn main() {
    config::init();
    logger::init();

    // FIXME: Add gracefull shutdown
    tokio::select! {
        Err(err) = monitor::start() => {
            error!(?err, "Monitor failed");
        }
        Err(err) = entrypoint::start() => {
            error!(?err, "Entrypoint failed");
        }
    }
}
