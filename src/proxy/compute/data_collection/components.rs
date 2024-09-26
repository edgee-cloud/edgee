wasmtime::component::bindgen!({world: "data-collection", path: "wit/protocols.wit"});

static WASM_LINKER: OnceCell<wasmtime::component::Linker<HostView>> = OnceCell::const_new();
static WASM_ENGINE: OnceCell<wasmtime::Engine> = OnceCell::const_new();
static WASM_COMPONENTS: OnceCell<HashMap<&str, Component>> = OnceCell::const_new();

use std::{collections::HashMap, str::FromStr};

use crate::config::config;
use crate::proxy::compute::data_collection::payload::{EventType, Payload};
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
    for cfg in &config::get().components.data_collection {
        let p = p.clone();
        let component = WASM_COMPONENTS
            .get()
            .unwrap()
            .get(cfg.name.as_str())
            .unwrap();
        let (instance, _) = DataCollection::instantiate(&mut store, &component, linker)?;
        let provider = instance.provider();
        let credentials: Vec<(String, String)> = cfg.credentials.clone().into_iter().collect();

        let payload = provider::Payload {
            uuid: p.uuid,
            timestamp: p.timestamp.timestamp(),
            event_type: match p.event_type {
                Some(EventType::Page) => provider::EventType::Page,
                Some(EventType::Identify) => provider::EventType::Identify,
                Some(EventType::Track) => provider::EventType::Track,
                _ => provider::EventType::Page,
            },
            page: provider::PageEvent {
                name: p.page.clone().unwrap().name.unwrap(),
                category: p.page.clone().unwrap().category.unwrap(),
                keywords: p.page.clone().unwrap().keywords.unwrap(),
                title: p.page.clone().unwrap().title.unwrap(),
                url: p.page.clone().unwrap().url.unwrap(),
                path: p.page.clone().unwrap().path.unwrap(),
                search: p.page.clone().unwrap().search.unwrap(),
                referrer: p.page.clone().unwrap().referrer.unwrap(),
                properties: p
                    .page
                    .clone()
                    .unwrap()
                    .properties
                    .unwrap()
                    .into_iter()
                    .map(|(key, value)| (key, value.to_string()))
                    .collect(),
            },
            identify: provider::IdentifyEvent {
                user_id: p.identify.clone().unwrap().user_id.unwrap(),
                anonymous_id: p.identify.clone().unwrap().anonymous_id.unwrap(),
                edgee_id: p.identify.clone().unwrap().edgee_id,
                properties: p
                    .identify
                    .clone()
                    .unwrap()
                    .properties
                    .unwrap()
                    .into_iter()
                    .map(|(key, value)| (key, value.to_string()))
                    .collect(),
            },
            track: provider::TrackEvent {
                name: p.track.clone().unwrap().name.unwrap(),
                properties: p
                    .track
                    .clone()
                    .unwrap()
                    .properties
                    .unwrap()
                    .into_iter()
                    .map(|(key, value)| (key, value.to_string()))
                    .collect(),
            },
            campaign: provider::Campaign {
                name: p.campaign.clone().unwrap().name.unwrap(),
                source: p.campaign.clone().unwrap().source.unwrap(),
                medium: p.campaign.clone().unwrap().medium.unwrap(),
                term: p.campaign.clone().unwrap().term.unwrap(),
                content: p.campaign.clone().unwrap().content.unwrap(),
                creative_format: p.campaign.clone().unwrap().creative_format.unwrap(),
                marketing_tactic: p.campaign.clone().unwrap().marketing_tactic.unwrap(),
            },
            client: provider::Client {
                ip: p.client.clone().unwrap().ip.unwrap(),
                x_forwarded_for: p.client.clone().unwrap().x_forwarded_for.unwrap(),
                locale: p.client.clone().unwrap().locale.unwrap(),
                timezone: p.client.clone().unwrap().timezone.unwrap(),
                user_agent: p.client.clone().unwrap().user_agent.unwrap(),
                user_agent_architecture: p.client.clone().unwrap().user_agent_architecture.unwrap(),
                user_agent_bitness: p.client.clone().unwrap().user_agent_bitness.unwrap(),
                user_agent_full_version_list: p
                    .client
                    .clone()
                    .unwrap()
                    .user_agent_full_version_list
                    .unwrap(),
                user_agent_mobile: p.client.clone().unwrap().user_agent_mobile.unwrap(),
                user_agent_model: p.client.clone().unwrap().user_agent_model.unwrap(),
                os_name: p.client.clone().unwrap().os_name.unwrap(),
                os_version: p.client.clone().unwrap().os_version.unwrap(),
                screen_width: p.client.clone().unwrap().screen_width.unwrap(),
                screen_height: p.client.clone().unwrap().screen_height.unwrap(),
                screen_density: p.client.clone().unwrap().screen_density.unwrap(),
                continent: String::new(),
                country_code: String::new(),
                country_name: String::new(),
                region: String::new(),
                city: String::new(),
            },
            session: provider::Session {
                session_id: p.session.clone().unwrap().session_id,
                previous_session_id: p.session.clone().unwrap().previous_session_id.unwrap(),
                session_count: p.session.clone().unwrap().session_count,
                session_start: p.session.clone().unwrap().session_start,
                first_seen: p.session.clone().unwrap().first_seen.to_string(),
                last_seen: p.session.clone().unwrap().last_seen.to_string(),
            },
            destinations: Vec::new(),
        };

        let request = match p.event_type {
            Some(EventType::Page) => provider.call_page(&mut store, &payload, &credentials),
            Some(EventType::Track) => provider.call_track(&mut store, &payload, &credentials),
            Some(EventType::Identify) => provider.call_identify(&mut store, &payload, &credentials),
            _ => Err(anyhow::anyhow!("invalid event type")),
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
