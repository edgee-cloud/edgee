wasmtime::component::bindgen!({world: "data-collection", path: "wit/protocols.wit"});

static WASM_LINKER: OnceCell<wasmtime::component::Linker<HostView>> = OnceCell::const_new();
static WASM_ENGINE: OnceCell<wasmtime::Engine> = OnceCell::const_new();
static WASM_COMPONENTS: OnceCell<HashMap<&str, Component>> = OnceCell::const_new();

use crate::config::config;
use crate::proxy::compute::data_collection::payload::{EventType, Payload};
use exports::provider;
use http::{HeaderMap, HeaderName, HeaderValue};
use std::net::IpAddr;
use std::time::Duration;
use std::{collections::HashMap, str::FromStr};
use tokio::sync::OnceCell;
use tracing::{error, info};
use wasmtime::component::Component;

pub fn init() {
    let mut runtime_conf = wasmtime::Config::default();
    runtime_conf.wasm_component_model(true);

    let engine = wasmtime::Engine::new(&runtime_conf).unwrap();
    let mut linker = wasmtime::component::Linker::<HostView>::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker).unwrap();

    let mut components: HashMap<&str, Component> = HashMap::new();
    for cfg in &config::get().components.data_collection {
        let component = Component::from_file(&engine, &cfg.component).unwrap();
        components.insert(&cfg.name, component);
    }

    if let Err(_) = WASM_ENGINE.set(engine) {
        panic!("failed to initialize wasm engine");
    }

    if let Err(_) = WASM_LINKER.set(linker) {
        panic!("failed to initialize wasm linker");
    }

    if let Err(_) = WASM_COMPONENTS.set(components) {
        panic!("failed to initialize wasm components");
    }
}

pub async fn send_data_collection(p: &Payload) -> anyhow::Result<()> {
    let engine = WASM_ENGINE.get().unwrap();
    let linker = WASM_LINKER.get().unwrap();
    let mut store = wasmtime::Store::new(engine, HostView::new());

    // clone the payload to be able to move it to the async thread
    let p = p.clone();
    let payload = provider::Payload {
        uuid: p.uuid,
        timestamp: p.timestamp.timestamp(),
        timestamp_millis: p.timestamp.timestamp_millis(),
        timestamp_micros: p.timestamp.timestamp_micros(),
        event_type: match p.event_type {
            Some(EventType::Page) => provider::EventType::Page,
            Some(EventType::Identify) => provider::EventType::Identify,
            Some(EventType::Track) => provider::EventType::Track,
            _ => provider::EventType::Page,
        },
        page: provider::PageEvent {
            name: p.page.clone().unwrap_or_default().name.unwrap_or_default(),
            category: p
                .page
                .clone()
                .unwrap_or_default()
                .category
                .unwrap_or_default(),
            keywords: p
                .page
                .clone()
                .unwrap_or_default()
                .keywords
                .unwrap_or_default(),
            title: p.page.clone().unwrap_or_default().title.unwrap_or_default(),
            url: p.page.clone().unwrap_or_default().url.unwrap_or_default(),
            path: p.page.clone().unwrap_or_default().path.unwrap_or_default(),
            search: p
                .page
                .clone()
                .unwrap_or_default()
                .search
                .unwrap_or_default(),
            referrer: p
                .page
                .clone()
                .unwrap_or_default()
                .referrer
                .unwrap_or_default(),
            properties: p
                .page
                .clone()
                .unwrap_or_default()
                .properties
                .unwrap_or_default()
                .into_iter()
                .map(|(key, value)| (key, value.to_string()))
                .collect(),
        },
        identify: provider::IdentifyEvent {
            user_id: p
                .identify
                .clone()
                .unwrap_or_default()
                .user_id
                .unwrap_or_default(),
            anonymous_id: p
                .identify
                .clone()
                .unwrap_or_default()
                .anonymous_id
                .unwrap_or_default(),
            edgee_id: p.identify.clone().unwrap_or_default().edgee_id,
            properties: p
                .identify
                .clone()
                .unwrap_or_default()
                .properties
                .unwrap_or_default()
                .into_iter()
                .map(|(key, value)| (key, value.to_string()))
                .collect(),
        },
        track: provider::TrackEvent {
            name: p.track.clone().unwrap_or_default().name.unwrap_or_default(),
            properties: p
                .track
                .clone()
                .unwrap_or_default()
                .properties
                .unwrap_or_default()
                .into_iter()
                .map(|(key, value)| (key, value.to_string()))
                .collect(),
        },
        campaign: provider::Campaign {
            name: p
                .campaign
                .clone()
                .unwrap_or_default()
                .name
                .unwrap_or_default(),
            source: p
                .campaign
                .clone()
                .unwrap_or_default()
                .source
                .unwrap_or_default(),
            medium: p
                .campaign
                .clone()
                .unwrap_or_default()
                .medium
                .unwrap_or_default(),
            term: p
                .campaign
                .clone()
                .unwrap_or_default()
                .term
                .unwrap_or_default(),
            content: p
                .campaign
                .clone()
                .unwrap_or_default()
                .content
                .unwrap_or_default(),
            creative_format: p
                .campaign
                .clone()
                .unwrap_or_default()
                .creative_format
                .unwrap_or_default(),
            marketing_tactic: p
                .campaign
                .clone()
                .unwrap_or_default()
                .marketing_tactic
                .unwrap_or_default(),
        },
        client: provider::Client {
            ip: anonymize_ip(&p.client.clone().unwrap_or_default().ip.unwrap_or_default()),
            locale: p
                .client
                .clone()
                .unwrap_or_default()
                .locale
                .unwrap_or_default(),
            timezone: p
                .client
                .clone()
                .unwrap_or_default()
                .timezone
                .unwrap_or_default(),
            user_agent: p
                .client
                .clone()
                .unwrap_or_default()
                .user_agent
                .unwrap_or_default(),
            user_agent_architecture: p
                .client
                .clone()
                .unwrap_or_default()
                .user_agent_architecture
                .unwrap_or_default(),
            user_agent_bitness: p
                .client
                .clone()
                .unwrap_or_default()
                .user_agent_bitness
                .unwrap_or_default(),
            user_agent_full_version_list: p
                .client
                .clone()
                .unwrap_or_default()
                .user_agent_full_version_list
                .unwrap_or_default(),
            user_agent_mobile: p
                .client
                .clone()
                .unwrap_or_default()
                .user_agent_mobile
                .unwrap_or_default(),
            user_agent_model: p
                .client
                .clone()
                .unwrap_or_default()
                .user_agent_model
                .unwrap_or_default(),
            os_name: p
                .client
                .clone()
                .unwrap_or_default()
                .os_name
                .unwrap_or_default(),
            os_version: p
                .client
                .clone()
                .unwrap_or_default()
                .os_version
                .unwrap_or_default(),
            screen_width: p
                .client
                .clone()
                .unwrap_or_default()
                .screen_width
                .unwrap_or_default(),
            screen_height: p
                .client
                .clone()
                .unwrap_or_default()
                .screen_height
                .unwrap_or_default(),
            screen_density: p
                .client
                .clone()
                .unwrap_or_default()
                .screen_density
                .unwrap_or_default(),
            continent: String::new(),
            country_code: String::new(),
            country_name: String::new(),
            region: String::new(),
            city: String::new(),
        },
        session: provider::Session {
            session_id: p.session.clone().unwrap_or_default().session_id,
            previous_session_id: p
                .session
                .clone()
                .unwrap_or_default()
                .previous_session_id
                .unwrap_or_default(),
            session_count: p.session.clone().unwrap_or_default().session_count,
            session_start: p.session.clone().unwrap_or_default().session_start,
            first_seen: p.session.clone().unwrap_or_default().first_seen.to_string(),
            last_seen: p.session.clone().unwrap_or_default().last_seen.to_string(),
        },
        destinations: Vec::new(),
    };

    for cfg in &config::get().components.data_collection {
        let component = WASM_COMPONENTS
            .get()
            .unwrap()
            .get(cfg.name.as_str())
            .unwrap();
        let (instance, _) = DataCollection::instantiate(&mut store, &component, linker)?;
        let provider = instance.provider();
        let credentials: Vec<(String, String)> = cfg.credentials.clone().into_iter().collect();

        let request = match p.event_type {
            Some(EventType::Page) => provider.call_page(&mut store, &payload, &credentials),
            Some(EventType::Track) => provider.call_track(&mut store, &payload, &credentials),
            Some(EventType::Identify) => provider.call_identify(&mut store, &payload, &credentials),
            _ => Err(anyhow::anyhow!("invalid event type")),
        };

        let event_str = match payload.event_type {
            provider::EventType::Page => "page",
            provider::EventType::Identify => "identify",
            provider::EventType::Track => "track",
        };

        match request {
            Ok(res) => match res {
                Ok(req) => {
                    let mut headers = HeaderMap::new();
                    for (key, value) in req.headers {
                        headers.insert(HeaderName::from_str(&key)?, HeaderValue::from_str(&value)?);
                    }
                    headers.insert(
                        HeaderName::from_str("x-forwarded-for")?,
                        HeaderValue::from_str(
                            p.client
                                .clone()
                                .unwrap_or_default()
                                .ip
                                .unwrap_or_default()
                                .as_str(),
                        )?,
                    );
                    headers.insert(
                        HeaderName::from_str("user-agent")?,
                        HeaderValue::from_str(
                            p.client
                                .clone()
                                .unwrap_or_default()
                                .user_agent
                                .unwrap_or_default()
                                .as_str(),
                        )?,
                    );

                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(5))
                        .build()?;

                    // spawn a separated async thread
                    tokio::spawn(async move {
                        let method_str = match req.method {
                            provider::HttpMethod::Get => "GET",
                            provider::HttpMethod::Put => "PUT",
                            provider::HttpMethod::Post => "POST",
                            provider::HttpMethod::Delete => "DELETE",
                        };
                        info!(target: "data_collection", step = "request", provider = cfg.name, event = event_str, method = method_str, url = req.url, body = req.body);
                        let res = match req.method {
                            provider::HttpMethod::Get => {
                                client.get(req.url).headers(headers).send().await
                            }
                            provider::HttpMethod::Put => {
                                client
                                    .put(req.url)
                                    .headers(headers)
                                    .body(req.body)
                                    .send()
                                    .await
                            }
                            provider::HttpMethod::Post => {
                                client
                                    .post(req.url)
                                    .headers(headers)
                                    .body(req.body)
                                    .send()
                                    .await
                            }
                            provider::HttpMethod::Delete => {
                                client.delete(req.url).headers(headers).send().await
                            }
                        };

                        match res {
                            Ok(res) => {
                                if res.status().is_success() {
                                    let status_str = format!("{:?}", res.status());
                                    let body_res_str = res.text().await.unwrap_or_default();
                                    info!(target: "data_collection", step = "response", provider = cfg.name, event = event_str, method = method_str, status = status_str, body = body_res_str);
                                } else {
                                    let status_str = format!("{:?}", res.status());
                                    let body_res_str = res.text().await.unwrap_or_default();
                                    error!(target: "data_collection", step = "response", provider = cfg.name, event = event_str, method = method_str, status = status_str, body = body_res_str);
                                }
                            }
                            Err(err) => {
                                error!(target: "data_collection", step = "response", provider = cfg.name, event = event_str, method = method_str, err = err.to_string());
                            }
                        }
                    });
                }
                Err(err) => {
                    error!(target: "data_collection", provider = cfg.name, event = event_str, err = err.to_string(), "failed to handle data collection payload");
                }
            },
            Err(err) => {
                error!(target: "data_collection", provider = cfg.name, event = event_str, err = err.to_string(), "failed to call data collection wasm component");
            }
        }
    }

    Ok(())
}

fn anonymize_ip(ip: &String) -> String {
    let mut ip = ip.parse::<IpAddr>().unwrap();
    const KEEP_IPV4_BYTES: usize = 3;
    const KEEP_IPV6_BYTES: usize = 6;

    ip = match ip {
        IpAddr::V4(ip) => {
            let mut o = ip.octets();
            o[KEEP_IPV4_BYTES..].copy_from_slice(&[0; 4 - KEEP_IPV4_BYTES]);
            IpAddr::V4(o.into())
        }
        IpAddr::V6(ip) => {
            let mut o = ip.octets();
            o[KEEP_IPV6_BYTES..].copy_from_slice(&[0; 16 - KEEP_IPV6_BYTES]);
            IpAddr::V6(o.into())
        }
    };
    ip.to_string()
}

struct HostView {
    table: wasmtime::component::ResourceTable,
    wasi: wasmtime_wasi::WasiCtx,
}

impl HostView {
    fn new() -> Self {
        let table = wasmtime_wasi::ResourceTable::new();
        let wasi = wasmtime_wasi::WasiCtxBuilder::new().build();
        Self { table, wasi }
    }
}

impl wasmtime_wasi::WasiView for HostView {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut wasmtime_wasi::WasiCtx {
        &mut self.wasi
    }
}
