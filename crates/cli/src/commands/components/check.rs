use edgee_components_runtime::config::{
    ComponentsConfiguration, ConsentMappingComponents, DataCollectionComponents,
};
use edgee_components_runtime::context::ComponentsContext;

#[derive(Debug, clap::Parser)]
pub struct Options {
    #[clap(short, long, num_args(1..))]
    pub data_collection_component: Option<Vec<String>>,

    #[clap(short, long, num_args(1..))]
    pub consent_mapping_component: Option<Vec<String>>,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    use crate::components::manifest::{self, Manifest};

    let (dc_components, cmp_components) = match (
        opts.data_collection_component.as_ref(),
        opts.consent_mapping_component.as_ref(),
    ) {
        (None, None) => {
            let manifest_path = manifest::find_manifest_path()
                .ok_or_else(|| anyhow::anyhow!("Manifest not found"))?;

            let manifest = Manifest::load(&manifest_path)?;
            // TODO: dont assume that it is a data collection component, add type in manifest
            (
                vec![manifest
                    .package
                    .build
                    .output_path
                    .into_os_string()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("Invalid path"))?],
                vec![],
            )
        }
        _ => (
            opts.data_collection_component.unwrap_or_default(),
            opts.consent_mapping_component.unwrap_or_default(),
        ),
    };

    // load a wasm host with a default configuration including the component we want to check
    let config = ComponentsConfiguration {
        data_collection: dc_components
            .into_iter()
            .map(|component_path| DataCollectionComponents {
                name: component_path.clone(),
                component: component_path,
                ..Default::default()
            })
            .collect(),
        consent_mapping: cmp_components
            .into_iter()
            .map(|component_path| ConsentMappingComponents {
                name: component_path.clone(),
                component: component_path,
                config: Default::default(),
            })
            .collect(),
        cache: None,
    };

    // check that the world is correctly implemented by components
    let context = ComponentsContext::new(&config)?;
    let mut store = context.empty_store();

    for name in context.components.data_collection.keys() {
        let _ = context
            .get_data_collection_instance(name, &mut store)
            .await?;
    }

    for name in context.components.consent_mapping.keys() {
        let _ = context
            .get_consent_mapping_instance(name, &mut store)
            .await?;
    }

    Ok(())
}
