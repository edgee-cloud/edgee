use std::collections::HashMap;

use wasmtime::{
    component::{Component, Linker},
    Engine, Store,
};
use wasmtime_wasi::{IoView, ResourceTable, WasiCtx, WasiView};

use crate::config::{ComponentsConfiguration, DataCollectionComponents};
use crate::consent_mapping::{ConsentMapping, ConsentMappingPre};
use crate::data_collection::v1_0_0::data_collection::{DataCollection, DataCollectionPre};
use crate::data_collection::version::DataCollectionWitVersion;

pub struct ComponentsContext {
    pub engine: Engine,
    pub components: Components,
}

pub struct Components {
    pub data_collection_1_0_0: HashMap<String, DataCollectionPre<HostState>>,
    pub consent_mapping: HashMap<String, ConsentMappingPre<HostState>>,
}

pub fn pre_instanciate_data_collection_component_internal(
    engine: &Engine,
    component_config: &DataCollectionComponents,
) -> anyhow::Result<DataCollectionPre<HostState>> {
    let mut linker = Linker::new(engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;

    let span = tracing::info_span!("component-context", component = %component_config.id, category = "data-collection");
    let _span = span.enter();

    tracing::debug!("Loading new data collection component");

    let component = Component::from_file(engine, &component_config.file)?;
    let instance_pre = linker.instantiate_pre(&component)?;
    let instance_pre = DataCollectionPre::new(instance_pre)?;

    tracing::debug!("loaded new data collection component");

    Ok(instance_pre)
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
        let data_collection_1_0_0_components = config
            .data_collection
            .iter()
            .filter(|entry| entry.wit_version == DataCollectionWitVersion::V1_0_0)
            .map(|entry| {
                let instance_pre =
                    pre_instanciate_data_collection_component_internal(&engine, entry)?;
                Ok((entry.id.clone(), instance_pre))
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
            data_collection_1_0_0: data_collection_1_0_0_components,
            consent_mapping: consent_mapping_components,
        };

        Ok(Self { engine, components })
    }

    pub fn pre_instanciate_data_collection_component(
        &self,
        component_config: DataCollectionComponents,
    ) -> anyhow::Result<DataCollectionPre<HostState>> {
        let instance_pre =
            pre_instanciate_data_collection_component_internal(&self.engine, &component_config)?;
        Ok(instance_pre)
    }

    pub fn add_data_collection_component(
        &mut self,
        component_config: DataCollectionComponents,
        instance_pre: DataCollectionPre<HostState>,
    ) {
        if !self
            .components
            .data_collection_1_0_0
            .contains_key(&component_config.id)
        {
            self.components
                .data_collection_1_0_0
                .insert(component_config.id.clone(), instance_pre);
        }
    }

    pub fn empty_store(&self) -> Store<HostState> {
        Store::new(&self.engine, HostState::new())
    }

    pub fn empty_store_with_stdout(&self) -> Store<HostState> {
        Store::new(&self.engine, HostState::new_with_stdout())
    }

    pub async fn get_data_collection_1_0_0_instance(
        &self,
        id: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<DataCollection> {
        let instance_pre = self.components.data_collection_1_0_0.get(id);

        if instance_pre.is_none() {
            return Err(anyhow::anyhow!("component not found: {}", id));
        }

        instance_pre.unwrap().instantiate_async(store).await
    }

    pub async fn get_consent_mapping_instance(
        &self,
        id: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<ConsentMapping> {
        let instance_pre = self.components.consent_mapping.get(id);

        if instance_pre.is_none() {
            return Err(anyhow::anyhow!("component not found: {}", id));
        }

        instance_pre.unwrap().instantiate_async(store).await
    }
}

pub struct HostState {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl HostState {
    fn new() -> Self {
        Self::new_with_ctx(WasiCtx::builder().build())
    }

    fn new_with_stdout() -> Self {
        Self::new_with_ctx(WasiCtx::builder().inherit_stdout().inherit_stderr().build())
    }

    fn new_with_ctx(ctx: WasiCtx) -> Self {
        let table = ResourceTable::new();
        Self { ctx, table }
    }
}

impl IoView for HostState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}
