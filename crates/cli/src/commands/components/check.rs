use colored::Colorize;

#[derive(Debug, clap::Parser)]
pub struct Options {
    #[arg(long = "filename")]
    filename: Option<String>,
}

pub enum ComponentType {
    DataCollection,
    #[allow(dead_code)]
    ConsentMapping,
}

pub async fn check_component(
    component_type: ComponentType,
    component_path: &str,
) -> anyhow::Result<()> {
    use edgee_components_runtime::config::{
        ComponentsConfiguration, ConsentMappingComponents, DataCollectionComponents,
    };
    use edgee_components_runtime::context::ComponentsContext;

    if !std::fs::exists(component_path)? {
        anyhow::bail!(
            "Component {} does not exist. Please run `edgee component build` first",
            component_path
        );
    }

    let config = match component_type {
        ComponentType::DataCollection => ComponentsConfiguration {
            data_collection: vec![DataCollectionComponents {
                id: component_path.to_string(),
                file: component_path.to_string(),
                ..Default::default()
            }],
            ..Default::default()
        },
        ComponentType::ConsentMapping => ComponentsConfiguration {
            consent_mapping: vec![ConsentMappingComponents {
                name: component_path.to_string(),
                component: component_path.to_string(),
                ..Default::default()
            }],
            ..Default::default()
        },
    };

    let context = ComponentsContext::new(&config)
        .map_err(|e| anyhow::anyhow!("Invalid component {}: {}", component_path, e))?;

    let mut store = context.empty_store();

    match component_type {
        ComponentType::DataCollection => {
            let _ = context
                .get_data_collection_instance(component_path, &mut store)
                .await?;
        }
        ComponentType::ConsentMapping => {
            let _ = context
                .get_consent_mapping_instance(component_path, &mut store)
                .await?;
        }
    }

    tracing::info!("Component {} is valid", component_path.green());
    Ok(())
}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use anyhow::Context;

    use crate::components::manifest::{self, Manifest};

    let component_path = match _opts.filename {
        Some(filename) => filename,
        None => {
            let Some(manifest_path) = manifest::find_manifest_path() else {
                anyhow::bail!("Edgee Manifest not found. Please run `edgee component new` and start from a template or `edgee component init` to create a new empty manifest in this folder.");
            };

            let manifest = Manifest::load(&manifest_path)?;
            let component_path = manifest
                .component
                .build
                .output_path
                .to_str()
                .context("Output path should be a valid UTF-8 string")?;
            component_path.to_string()
        }
    };

    // TODO: dont assume that it is a data collection component, add type in manifest
    check_component(ComponentType::DataCollection, component_path.as_str()).await?;

    Ok(())
}
