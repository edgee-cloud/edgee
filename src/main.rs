use tracing::error;

mod config;
mod monitor;

#[tokio::main]
async fn main() {
    config::init();

    tokio::select! {
        Err(err) = monitor::start() => {
            error!(?err, "Monitor failed");
        }
    }
}
