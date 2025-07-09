use crate::{config::EdgeFunctionComponents, context::ComponentsContext};
use wasmtime::{
    component::{Component, Linker},
    Engine, Store,
};

use crate::context::HostState;
use crate::edge_function::versions::v1_0_0::edge_function::EdgeFunctionV100;
use crate::edge_function::versions::v1_0_0::edge_function::EdgeFunctionV100Pre;

pub mod edge_function {
    wasmtime::component::bindgen!({
        world: "edge-function-v100",
        path: "src/edge_function/wit",
        async: true,
        with: {
            "wasi:http@0.2.0": wasmtime_wasi_http::bindings::http,
        },
        trappable_imports: true,
    });
}

pub fn pre_instanciate_edge_function_component_1_0_0(
    engine: &Engine,
    component_config: &EdgeFunctionComponents,
) -> anyhow::Result<EdgeFunctionV100Pre<HostState>> {
    let mut linker = Linker::new(engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;
    let span = tracing::info_span!("component-context", component = %component_config.id, category = "edge-function");
    let _span = span.enter();

    tracing::debug!("Start pre-instantiate edge-function component");

    // try to load from serialized file if available
    let component = match component_config.serialized_file {
        Some(serialized_file) => {
            tracing::debug!(
                "Loading edge-function component from serialized file: {}",
                serialized_file
            );
            unsafe { Component::deserialize(engine, &serialized_file) }.ok()
        }
        None => None,
    }
    .unwrap_or_else(|| {
        tracing::debug!(
            "Loading edge-function component from file: {}",
            component_config.file
        );
        Component::from_file(engine, &component_config.file)
    })?;

    let instance_pre = linker.instantiate_pre(&component)?;
    let instance_pre = EdgeFunctionV100Pre::new(instance_pre)?;
    tracing::debug!("Finished pre-instantiate edge-function component");

    Ok(instance_pre)
}

impl ComponentsContext {
    pub async fn get_edge_function_1_0_0_instance(
        &self,
        id: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<EdgeFunctionV100> {
        let instance_pre = self.components.edge_function_1_0_0.get(id);

        if instance_pre.is_none() {
            return Err(anyhow::anyhow!("component not found: {}", id));
        }

        instance_pre.unwrap().instantiate_async(store).await
    }

    pub fn add_edge_function_1_0_0_instance(
        &mut self,
        component_config: EdgeFunctionComponents,
        instance_pre: EdgeFunctionV100Pre<HostState>,
    ) {
        if !self
            .components
            .edge_function_1_0_0
            .contains_key(&component_config.id)
        {
            self.components
                .edge_function_1_0_0
                .insert(component_config.id.clone(), instance_pre);
        }
    }
}
