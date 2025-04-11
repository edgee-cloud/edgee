use crate::context::ComponentsContext;
use wasmtime::Store;

use crate::consent_management::versions::v1_0_0::consent_management::ConsentManagementV100;
use crate::context::HostState;

pub mod consent_management {
    wasmtime::component::bindgen!({
        world: "consent-management-v100",
        path: "src/consent_management/wit",
        async: true,
    });
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
