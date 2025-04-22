use crate::{config::ConsentManagementComponents, context::ComponentsContext};
use wasmtime::{
    component::{Component, Linker},
    Engine, Store,
};

use crate::consent_management::versions::v1_0_0::consent_management::ConsentManagementV100;
use crate::consent_management::versions::v1_0_0::consent_management::ConsentManagementV100Pre;
use crate::context::HostState;

pub mod consent_management {
    wasmtime::component::bindgen!({
        world: "consent-management-v100",
        path: "src/consent_management/wit",
        async: true,
    });
}

pub fn pre_instanciate_consent_management_component_1_0_0(
    engine: &Engine,
    component_config: &ConsentManagementComponents,
) -> anyhow::Result<ConsentManagementV100Pre<HostState>> {
    let mut linker = Linker::new(engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;
    let span = tracing::info_span!("component-context", component = %component_config.id, category = "consent-management");
    let _span = span.enter();

    tracing::debug!("Start pre-instantiate consent management component");

    let component = Component::from_file(engine, &component_config.file)?;
    let instance_pre = linker.instantiate_pre(&component)?;
    let instance_pre = ConsentManagementV100Pre::new(instance_pre)?;

    tracing::debug!("Finished pre-instantiate consent management component");

    Ok(instance_pre)
}

impl ComponentsContext {
    pub async fn get_consent_management_1_0_0_instance(
        &self,
        id: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<ConsentManagementV100> {
        let instance_pre = self.components.consent_management_1_0_0.get(id);

        if instance_pre.is_none() {
            return Err(anyhow::anyhow!("component not found: {}", id));
        }

        instance_pre.unwrap().instantiate_async(store).await
    }
}
