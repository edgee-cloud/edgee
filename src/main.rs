mod config {

    use serde::Deserialize;
    use tokio::sync::OnceCell;

    static CONFIG: OnceCell<StaticConfiguration> = OnceCell::const_new();

    #[derive(Deserialize, Debug)]
    pub struct StaticConfiguration {
        pub log: LogConfiguration,
        pub entrypoints: Vec<EntryPointConfiguration>,
    }

    #[derive(Deserialize, Debug)]
    pub struct LogConfiguration {
        pub level: String,
    }

    #[derive(Deserialize, Debug)]
    pub struct EntryPointConfiguration {
        pub name: String,
        pub bind: String,
        pub domains: Vec<DomainConfiguration>,
    }

    #[derive(Deserialize, Debug)]
    pub struct DomainConfiguration {
        pub host: String,
    }

    pub fn init() {
        let config_file = std::fs::read_to_string("edgee.toml").unwrap();
        let config: StaticConfiguration = toml::from_str(&config_file).unwrap();
        CONFIG.set(config).unwrap();
    }

    pub fn get() -> &'static StaticConfiguration {
        CONFIG.get().unwrap()
    }
}

mod logger {
    use tracing_subscriber::{fmt::Subscriber, util::SubscriberInitExt, EnvFilter};

    use crate::config;

    const ACCEPTED_LEVELS: [&str; 6] = ["trace", "debug", "info", "warn", "error", "fatal"];

    pub fn init() {
        let level = &config::get().log.level;

        if !ACCEPTED_LEVELS.contains(&level.as_str()) {
            panic!("Unsupported log level: {level}");
        }

        let filter: EnvFilter = level.into();
        Subscriber::builder()
            .with_env_filter(filter)
            .finish()
            .try_init()
            .unwrap();
    }
}

mod entrypoints {

    use std::net::{SocketAddr, ToSocketAddrs};

    use anyhow::Result;
    use tokio::net::TcpListener;
    use tokio::task::JoinSet;
    use tracing::debug;

    use crate::config;
    use crate::domains;

    pub async fn start() -> Result<()> {
        let mut joinset = JoinSet::new();

        for cfg in &config::get().entrypoints {
            debug!(name = cfg.name, binding = cfg.bind, "starting entrypoint");
            let addr: SocketAddr = cfg
                .bind
                .to_socket_addrs()
                .unwrap()
                .next()
                .expect("Valid socket address");

            let listener = TcpListener::bind(addr).await.unwrap();
            joinset.spawn(async move {
                loop {
                    let (stream, addr) = listener.accept().await.unwrap();
                    domains::respond(&cfg.domains, stream, addr);
                }
            });
        }

        let Some(result) = joinset.join_next().await else {
            todo!();
        };

        result?
    }
}

mod domains {
    use std::net::SocketAddr;

    use anyhow::Result;
    use bytes::Bytes;
    use http::{Request, Response, StatusCode};
    use http_body_util::{combinators::BoxBody, BodyExt, Empty};
    use hyper::{body::Incoming, service::service_fn};
    use hyper_util::{
        rt::{TokioExecutor, TokioIo},
        server::conn::auto::Builder,
    };
    use tokio::net::TcpStream;
    use tracing::error;

    use crate::config;

    pub fn respond(_cfg: &[config::DomainConfiguration], stream: TcpStream, _addr: SocketAddr) {
        async fn handle_request(
            _req: Request<Incoming>,
        ) -> Result<Response<BoxBody<Bytes, hyper::Error>>> {
            Ok(Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(
                    Empty::<Bytes>::new()
                        .map_err(|never| match never {})
                        .boxed(),
                )
                .unwrap())
        }

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            Builder::new(TokioExecutor::new())
                .serve_connection(io, service_fn(handle_request))
                .await
                .map_err(|err| error!(?err, "Failed to serve connection"))
        });
    }
}

#[tokio::main]
async fn main() {
    config::init();
    logger::init();
    let _ = entrypoints::start().await;
}
