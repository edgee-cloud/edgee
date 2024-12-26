setup_command! {}

pub async fn run(_opts: Options) {
    edgee_server::init().await.unwrap();

    tokio::select! {
        Err(err) = edgee_server::monitor::start() => tracing::error!(?err, "Monitor failed"),
        Err(err) = edgee_server::start() => tracing::error!(?err, "Server failed to start"),
    }
}
