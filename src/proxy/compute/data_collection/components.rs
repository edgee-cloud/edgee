wasmtime::component::bindgen!({world: "data-collection", path: "wit/protocols.wit"});

static WASM_LINKER: OnceCell<wasmtime::component::Linker<HostView>> = OnceCell::const_new();
static WASM_ENGINE: OnceCell<wasmtime::Engine> = OnceCell::const_new();
static WASM_COMPONENTS: OnceCell<HashMap<&str, Component>> = OnceCell::const_new();

use std::{collections::HashMap, str::FromStr};

use crate::config::config;
use crate::proxy::compute::data_collection::data_collection::{EventType, Payload};
use exports::provider;
use http::{HeaderMap, HeaderName, HeaderValue};
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
    for cfg in &config::get().destinations.data_collection {
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

pub async fn send_data_collection(p: Payload) -> anyhow::Result<()> {
    let engine = WASM_ENGINE.get().unwrap();
    let linker = WASM_LINKER.get().unwrap();
    let mut store = wasmtime::Store::new(engine, HostView::new());
    for cfg in &config::get().destinations.data_collection {
        let p = p.clone();
        let component = WASM_COMPONENTS
            .get()
            .unwrap()
            .get(cfg.name.as_str())
            .unwrap();
        let (instance, _) = DataCollection::instantiate(&mut store, &component, linker).unwrap();
        let provider = instance.provider();
        let credentials: Vec<(String, String)> = cfg.credentials.clone().into_iter().collect();

        let payload = provider::Payload {
            uuid: p.uuid,
            timestamp: p.timestamp.timestamp(),
            event_type: match p.event_type {
                EventType::Page => provider::EventType::Page,
                EventType::Identify => provider::EventType::Identify,
                EventType::Track => provider::EventType::Track,
            },
            page: provider::PageEvent {
                name: p.page.name,
                category: p.page.category,
                keywords: p.page.keywords,
                title: p.page.title,
                url: p.page.url,
                path: p.page.path,
                search: p.page.search,
                referrer: p.page.referrer,
                properties: p
                    .page
                    .properties
                    .into_iter()
                    .map(|(key, value)| (key, value.to_string()))
                    .collect(),
            },
            identify: provider::IdentifyEvent {
                user_id: p.identify.user_id,
                anonymous_id: p.identify.anonymous_id,
                edgee_id: p.identify.edgee_id,
                properties: p
                    .identify
                    .properties
                    .into_iter()
                    .map(|(key, value)| (key, value.to_string()))
                    .collect(),
            },
            track: provider::TrackEvent {
                name: p.track.clone().map(|t| t.name).unwrap_or_default(),
                properties: p
                    .track
                    .map(|t| {
                        t.properties
                            .into_iter()
                            .map(|(key, value)| (key, value.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            },
            campaign: provider::Campaign {
                name: p.campaign.name,
                source: p.campaign.source,
                medium: p.campaign.medium,
                term: p.campaign.term,
                content: p.campaign.content,
                creative_format: p.campaign.creative_format,
                marketing_tactic: p.campaign.marketing_tactic,
            },
            client: provider::Client {
                ip: p.client.ip,
                x_forward_for: p.client.x_forwarded_for,
                locale: p.client.locale,
                timezone: p.client.timezone.to_string(),
                user_agent: p.client.user_agent,
                user_agent_architecture: p.client.user_agent_architecture,
                user_agent_bitness: p.client.user_agent_bitness,
                user_agent_full_version_list: p.client.user_agent_full_version_list,
                user_agent_mobile: p.client.user_agent_mobile,
                user_agent_model: p.client.user_agent_model,
                os_name: p.client.os_name,
                os_version: p.client.os_version,
                screen_width: p.client.screen_width,
                screen_height: p.client.screen_height,
                screen_density: p.client.screen_density,
                continent: String::new(),
                country_code: String::new(),
                country_name: String::new(),
                region: String::new(),
                city: String::new(),
            },
            session: provider::Session {
                session_id: p.session.session_id,
                previous_session_id: p.session.previous_session_id,
                session_count: p.session.session_count,
                session_start: p.session.session_start,
                first_seen: p.session.first_seen.to_string(),
                last_seen: p.session.last_seen.to_string(),
            },
            destinations: Vec::new(),
        };

        let request = match p.event_type {
            EventType::Page => provider.call_page(&mut store, &payload, &credentials),
            EventType::Track => provider.call_track(&mut store, &payload, &credentials),
            EventType::Identify => provider.call_identify(&mut store, &payload, &credentials),
        };

        match request {
            Ok(res) => match res {
                Ok(req) => {
                    let mut headers = HeaderMap::new();
                    for (key, value) in req.headers {
                        headers.insert(
                            HeaderName::from_str(&key).unwrap(),
                            HeaderValue::from_str(&value).unwrap(),
                        );
                    }
                    let client = reqwest::Client::new();
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
                                info!(
                                    provider = cfg.name,
                                    event = ?payload.event_type,
                                    "request sent successfully"
                                );
                            } else {
                                error!(provider = cfg.name, event = ?payload.event_type,"request failed with status: {}", res.status());
                                println!("{:?}", res.text().await);
                            }
                        }
                        Err(err) => {
                            error!(?err, provider = cfg.name, event = ?payload.event_type, "failed to send request")
                        }
                    }
                }
                Err(err) => {
                    error!(?err, provider = cfg.name, event = ?payload.event_type, "failed to handle payload")
                }
            },
            Err(err) => {
                error!(?err, provider = cfg.name, event = ?payload.event_type, "failed to call wasm component")
            }
        }
    }

    Ok(())
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
