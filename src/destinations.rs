wasmtime::component::bindgen!({world: "data-collection", path: "wit/protocols.wit"});

static WASM_LINKER: OnceCell<wasmtime::component::Linker<HostView>> = OnceCell::const_new();
static WASM_ENGINE: OnceCell<wasmtime::Engine> = OnceCell::const_new();

use std::collections::HashMap;

use exports::provider::Guest;
use tokio::sync::OnceCell;
use wasmtime::component::Component;

use crate::{
    config,
    data_collection::{EventType, Payload},
};

pub fn init() {
    let mut runtime_conf = wasmtime::Config::default();
    runtime_conf.wasm_component_model(true);

    let engine = wasmtime::Engine::new(&runtime_conf).unwrap();
    let mut linker = wasmtime::component::Linker::<HostView>::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker).unwrap();
    wasmtime_wasi_http::proxy::add_only_http_to_linker(&mut linker).unwrap();

    if let Err(_) = WASM_ENGINE.set(engine) {
        panic!("failed to initialize wasm engine");
    }

    if let Err(_) = WASM_LINKER.set(linker) {
        panic!("failed to initialize wasm linker");
    }
}

pub async fn send_data_collection(p: &Payload) -> anyhow::Result<()> {
    let engine = WASM_ENGINE.get().unwrap();
    let linker = WASM_LINKER.get().unwrap();
    let mut store = wasmtime::Store::new(engine, HostView::new());
    let data_collection = &config::get().destinations.data_collection;
    for cfg in data_collection {
        let component = Component::from_file(engine, &cfg.component).unwrap();
        let (instance, _) = DataCollection::instantiate(&mut store, &component, linker).unwrap();
        let _provider = instance.provider();
        match p.event_type {
            EventType::Page => println!("page"),
            EventType::Track => println!("track"),
            EventType::Identify => println!("identify"),
        }
    }

    Ok(())
}

struct HostView {
    table: wasmtime::component::ResourceTable,
    wasi: wasmtime_wasi::WasiCtx,
    http: wasmtime_wasi_http::WasiHttpCtx,
}

impl HostView {
    fn new() -> Self {
        let table = wasmtime_wasi::ResourceTable::new();
        let wasi = wasmtime_wasi::WasiCtxBuilder::new().build();
        let http = wasmtime_wasi_http::WasiHttpCtx::new();
        Self { table, wasi, http }
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

impl wasmtime_wasi_http::WasiHttpView for HostView {
    fn ctx(&mut self) -> &mut wasmtime_wasi_http::WasiHttpCtx {
        &mut self.http
    }

    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        &mut self.table
    }
}

struct DataCollectionProvider {
    binding: &'static Guest,
    credentials: &'static HashMap<String, String>,
}

impl DataCollectionProvider {
    fn send(&self, payload: &Payload) -> anyhow::Result<()> {
        Ok(())
    }
}
