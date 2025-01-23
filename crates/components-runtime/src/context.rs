use std::collections::HashMap;

use wasmtime::{
    component::{Component, Linker, ResourceTable},
    Engine, Store,
};
use wasmtime_wasi::{WasiCtx, WasiView};

use crate::config::ComponentsConfiguration;
use crate::consent_mapping::{ConsentMapping, ConsentMappingPre};
use crate::data_collection::{DataCollection, DataCollectionPre};
pub struct ComponentsContext {
    pub engine: Engine,
    pub components: Components,
}

pub struct Components {
    pub data_collection: HashMap<String, DataCollectionPre<HostState>>,
    pub consent_mapping: HashMap<String, ConsentMappingPre<HostState>>,
}

impl ComponentsContext {
    pub fn new(config: &ComponentsConfiguration) -> anyhow::Result<Self> {
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

        // Data collection components
        let data_collection_components = config
            .data_collection
            .iter()
            .map(|entry| {
                let span = tracing::info_span!("component-context", component = %entry.name, category = "data-collection");
                let _span = span.enter();

                tracing::debug!("Start pre-instantiate data collection component");

                let component = Component::from_file(&engine, &entry.component)?;
                let instance_pre = linker.instantiate_pre(&component)?;
                let instance_pre = DataCollectionPre::new(instance_pre)?;

                tracing::debug!("Finished pre-instantiate data collection component");

                Ok((entry.name.clone(), instance_pre))
            })
            .collect::<anyhow::Result<_>>()?;

        // Consent mapping components
        let consent_mapping_components = config
            .consent_mapping
            .iter()
            .map(|entry| {
                let span = tracing::info_span!("component-context", component = %entry.name, category = "consent-mapping");
                let _span = span.enter();

                tracing::debug!("Start pre-instantiate consent mapping component");

                let component = Component::from_file(&engine, &entry.component)?;
                let instance_pre = linker.instantiate_pre(&component)?;
                let instance_pre = ConsentMappingPre::new(instance_pre)?;

                tracing::debug!("Finished pre-instantiate consent mapping component");

                Ok((entry.name.clone(), instance_pre))
            })
            .collect::<anyhow::Result<_>>()?;

        let components = Components {
            data_collection: data_collection_components,
            consent_mapping: consent_mapping_components,
        };

        Ok(Self { engine, components })
    }

    pub fn empty_store(&self) -> Store<HostState> {
        Store::new(&self.engine, HostState::new())
    }

    pub async fn get_data_collection_instance(
        &self,
        name: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<DataCollection> {
        let instance_pre = self
            .components
            .data_collection
            .get(name)
            .expect("Data collection component not found");

        instance_pre
            .instantiate_async(store)
            .await
            .map_err(Into::into)
    }

    pub async fn get_consent_mapping_instance(
        &self,
        name: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<ConsentMapping> {
        let instance_pre = self
            .components
            .consent_mapping
            .get(name)
            .expect("Consent mapping component not found");

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
