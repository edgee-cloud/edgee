use tracing::error;

mod config;
mod logger;
mod monitor;

#[tokio::main]
async fn main() {
    config::init();
    logger::init();

    tokio::select! {
        Err(err) = monitor::start() => {
            error!(?err, "Monitor failed");
        }
    }
}
