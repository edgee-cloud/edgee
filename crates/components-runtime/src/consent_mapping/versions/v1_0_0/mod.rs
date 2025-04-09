use crate::context::ComponentsContext;
use wasmtime::Store;

use crate::consent_mapping::versions::v1_0_0::consent_mapping::ConsentMappingV100;
use crate::context::HostState;

pub mod consent_mapping {
    wasmtime::component::bindgen!({
        world: "consent-mapping-v100",
        path: "src/consent_mapping/wit",
        async: true,
    });
}

impl ComponentsContext {
    pub async fn get_consent_mapping_1_0_0_instance(
        &self,
        id: &str,
        store: &mut Store<HostState>,
    ) -> anyhow::Result<ConsentMappingV100> {
        let instance_pre = self.components.consent_mapping_1_0_0.get(id);

        if instance_pre.is_none() {
            return Err(anyhow::anyhow!("component not found: {}", id));
        }

        instance_pre.unwrap().instantiate_async(store).await
    }
}
