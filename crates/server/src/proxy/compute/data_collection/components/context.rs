use std::collections::HashMap;

use tokio::sync::OnceCell;
use wasmtime::{
    component::{Component, Linker, ResourceTable},
    Engine, Store,
};
use wasmtime_wasi::{WasiCtx, WasiView};

use super::{DataCollection, DataCollectionPre};
use crate::config;

static COMPONENTS_CONTEXT: OnceCell<ComponentsContext> = OnceCell::const_new();

pub struct ComponentsContext {
    pub engine: Engine,
    pub components: HashMap<String, DataCollectionPre<HostState>>,
}

impl ComponentsContext {
    fn new() -> anyhow::Result<Self> {
        let config = &config::get().components;

        let mut engine_config = wasmtime::Config::new();
        engine_config
            .wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable)
            .wasm_component_model(true)
            .async_support(true);

        if let Some(path) = config.cache.as_deref() {
            engine_config.cache_config_load(path)?;
        };

        let engine = Engine::new(&engine_config)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        let config = config::get();
        let components = config
            .components
            .data_collection
            .iter()
            .map(|entry| {
                let span = tracing::info_span!("component-context", component = %entry.name);
                let _span = span.enter();

                tracing::debug!("Start pre-instanciate component");

                let component = Component::from_file(&engine, &entry.component)?;
                let instance_pre = linker.instantiate_pre(&component)?;
                let instance_pre = DataCollectionPre::new(instance_pre)?;

                tracing::debug!("Finished pre-instanciate component");

                Ok((entry.name.clone(), instance_pre))
            })
            .collect::<anyhow::Result<_>>()?;

        Ok(Self { engine, components })
    }

    pub fn init() -> anyhow::Result<()> {
        let ctx = Self::new()?;

        COMPONENTS_CONTEXT
            .set(ctx)
            .map_err(|err| anyhow::anyhow!("Failed to register ComponentsContext: {err}"))
    }

    pub fn get() -> &'static ComponentsContext {
        COMPONENTS_CONTEXT
            .get()
            .expect("ComponentsContext should be registered")
    }

    pub fn empty_store(&self) -> Store<HostState> {
        Store::new(&self.engine, HostState::new())
    }

    pub async fn instantiate_data_collection(
        &self,
        name: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<DataCollection> {
        let instance_pre = self
            .components
            .get(name)
            .expect("Data collection not found, should not happen");

        instance_pre
            .instantiate_async(store)
            .await
            .map_err(Into::into)
    }
}

pub struct HostState {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl HostState {
    fn new() -> Self {
        let ctx = WasiCtx::builder().build();

        let table = ResourceTable::new();

        Self { ctx, table }
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}