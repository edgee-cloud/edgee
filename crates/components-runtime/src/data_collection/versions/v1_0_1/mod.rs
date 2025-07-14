mod convert;
pub mod execute;
use crate::config::DataCollectionComponents;
use crate::context::ComponentsContext;
use crate::context::HostState;
use crate::data_collection::versions::v1_0_1::data_collection::{
    DataCollectionV101, DataCollectionV101Pre,
};
use wasmtime::{
    component::{Component, Linker},
    Engine, Store,
};

pub mod data_collection {
    wasmtime::component::bindgen!({
        world: "data-collection-v101",
        path: "src/data_collection/wit",
        async: true,
    });
}

pub fn pre_instanciate_data_collection_component_1_0_1(
    engine: &Engine,
    component_config: &DataCollectionComponents,
) -> anyhow::Result<DataCollectionV101Pre<HostState>> {
    let mut linker = Linker::new(engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;

    let span = tracing::info_span!("component-context", component = %component_config.id, category = "data-collection");
    let _span = span.enter();

    tracing::debug!("Loading new data collection component");

    let component = Component::from_file(engine, &component_config.file)?;
    let instance_pre = linker.instantiate_pre(&component)?;
    let instance_pre = DataCollectionV101Pre::new(instance_pre)?;

    tracing::debug!("loaded new data collection component");

    Ok(instance_pre)
}

impl ComponentsContext {
    pub fn serialize_data_collection_1_0_1(&self, id: &str) -> anyhow::Result<Vec<u8>> {
        let instance_pre = self.components.data_collection_1_0_1.get(id);

        match instance_pre {
            Some(instance) => Ok(instance.instance_pre().component().serialize()?),
            None => Err(anyhow::anyhow!("component not found: {}", id)),
        }
    }

    pub fn pre_instanciate_data_collection_1_0_1_component(
        &self,
        component_config: DataCollectionComponents,
    ) -> anyhow::Result<DataCollectionV101Pre<HostState>> {
        let instance_pre =
            pre_instanciate_data_collection_component_1_0_1(&self.engine, &component_config)?;
        Ok(instance_pre)
    }

    pub fn add_data_collection_1_0_1_component(
        &mut self,
        component_config: DataCollectionComponents,
        instance_pre: DataCollectionV101Pre<HostState>,
    ) {
        if !self
            .components
            .data_collection_1_0_1
            .contains_key(&component_config.id)
        {
            self.components
                .data_collection_1_0_1
                .insert(component_config.id.clone(), instance_pre);
        }
    }

    pub async fn get_data_collection_1_0_1_instance(
        &self,
        id: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<DataCollectionV101> {
        let instance_pre = self.components.data_collection_1_0_1.get(id);

        if instance_pre.is_none() {
            return Err(anyhow::anyhow!("component not found: {}", id));
        }

        instance_pre.unwrap().instantiate_async(store).await
    }
}
