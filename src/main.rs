mod config;

#[tokio::main]
async fn main() {
    config::init();
}
