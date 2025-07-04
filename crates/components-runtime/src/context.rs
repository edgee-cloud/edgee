use std::collections::HashMap;

use crate::config::ComponentsConfiguration;
use crate::data_collection::versions::v1_0_0::data_collection::DataCollectionV100Pre;
use crate::data_collection::versions::v1_0_0::pre_instanciate_data_collection_component_1_0_0;
use crate::data_collection::versions::v1_0_1::data_collection::DataCollectionV101Pre;
use crate::data_collection::versions::v1_0_1::pre_instanciate_data_collection_component_1_0_1;
use crate::data_collection::versions::DataCollectionWitVersion;
use wasmtime::{Engine, Store};
use wasmtime_wasi::{IoView, ResourceTable, WasiCtx, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::edge_function::versions::v1_0_0::edge_function::EdgeFunctionV100Pre;
use crate::edge_function::versions::v1_0_0::pre_instanciate_edge_function_component_1_0_0;
use crate::edge_function::versions::EdgeFunctionWitVersion;

#[derive(Clone)]
pub struct ComponentsContext {
    pub engine: Engine,
    pub components: Components,
}

#[derive(Clone)]
pub struct Components {
    pub data_collection_1_0_0: HashMap<String, DataCollectionV100Pre<HostState>>,
    pub data_collection_1_0_1: HashMap<String, DataCollectionV101Pre<HostState>>,
    pub edge_function_1_0_0: HashMap<String, EdgeFunctionV100Pre<HostState>>,
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
        } else {
            // try to load the default cache
            let _ = engine_config.cache_config_load_default();
        }

        let engine = Engine::new(&engine_config)?;

        // Data collection components
        let data_collection_1_0_0_components = config
            .data_collection
            .iter()
            .filter(|entry| entry.wit_version == DataCollectionWitVersion::V1_0_0)
            .map(|entry| {
                let instance_pre = pre_instanciate_data_collection_component_1_0_0(&engine, entry)?;
                Ok((entry.id.clone(), instance_pre))
            })
            .collect::<anyhow::Result<_>>()?;

        let data_collection_1_0_1_components = config
            .data_collection
            .iter()
            .filter(|entry| entry.wit_version == DataCollectionWitVersion::V1_0_1)
            .map(|entry| {
                let instance_pre = pre_instanciate_data_collection_component_1_0_1(&engine, entry)?;
                Ok((entry.id.clone(), instance_pre))
            })
            .collect::<anyhow::Result<_>>()?;

        let edge_function_1_0_0_components = config
            .edge_function
            .iter()
            .filter(|entry| entry.wit_version == EdgeFunctionWitVersion::V1_0_0)
            .map(|entry| {
                let instance_pre = pre_instanciate_edge_function_component_1_0_0(&engine, entry)?;
                Ok((entry.id.clone(), instance_pre))
            })
            .collect::<anyhow::Result<_>>()?;

        let components = Components {
            data_collection_1_0_0: data_collection_1_0_0_components,
            data_collection_1_0_1: data_collection_1_0_1_components,
            edge_function_1_0_0: edge_function_1_0_0_components,
        };

        Ok(Self { engine, components })
    }

    pub fn empty_store(&self) -> Store<HostState> {
        Store::new(&self.engine, HostState::new())
    }

    pub fn empty_store_with_stdout(&self) -> Store<HostState> {
        Store::new(&self.engine, HostState::new_with_stdout())
    }
}

pub struct HostState {
    ctx: WasiCtx,
    table: ResourceTable,
    http: WasiHttpCtx,
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
        let http = WasiHttpCtx::new();
        Self { ctx, table, http }
    }
}

impl WasiHttpView for HostState {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
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
