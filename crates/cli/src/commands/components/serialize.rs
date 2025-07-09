use edgee_components_runtime::config::EdgeFunctionComponents;

setup_command! {
    #[arg(long = "filename")]
    filename: Option<String>,
    #[arg(long = "component-type")]
    component_type: Option<String>,
    #[arg(long = "wit-version")]
    wit_version: Option<String>,
}

pub enum ComponentType {
    DataCollection,
    EdgeFunction,
}

pub async fn serialize_component(
    component_type: ComponentType,
    component_path: &str,
    component_wit_version: &str,
) -> anyhow::Result<()> {
    use edgee_components_runtime::config::{ComponentsConfiguration, DataCollectionComponents};
    use edgee_components_runtime::context::ComponentsContext;

    if !std::fs::exists(component_path)? {
        anyhow::bail!(
            "Component {} does not exist. Please run `edgee component build` first",
            component_path
        );
    }

    let config = match component_type {
        ComponentType::DataCollection => match component_wit_version {
            "1.0.0" => ComponentsConfiguration {
                data_collection: vec![DataCollectionComponents {
                    id: component_path.to_string(),
                    file: component_path.to_string(),
                    wit_version: edgee_components_runtime::data_collection::versions::DataCollectionWitVersion::V1_0_0,
                    ..Default::default()
                }],
                ..Default::default()
            },
            "1.0.1" => ComponentsConfiguration {
                data_collection: vec![DataCollectionComponents {
                    id: component_path.to_string(),
                    file: component_path.to_string(),
                    wit_version: edgee_components_runtime::data_collection::versions::DataCollectionWitVersion::V1_0_1,
                    ..Default::default()
                }],
                ..Default::default()
            },
            _ => anyhow::bail!("Invalid WIT version: {}", component_wit_version),
        },
        ComponentType::EdgeFunction => match component_wit_version {
            "1.0.0" => ComponentsConfiguration {
                edge_function: vec![EdgeFunctionComponents{
                    id: component_path.to_string(),
                    file: component_path.to_string(),
                    wit_version: edgee_components_runtime::edge_function::versions::EdgeFunctionWitVersion::V1_0_0,
                    ..Default::default()
                }],
                ..Default::default()
            },
            _ => anyhow::bail!("Invalid WIT version: {}", component_wit_version),
        }
    };

    let context = ComponentsContext::new(&config)
        .map_err(|e| anyhow::anyhow!("Invalid component {}: {}", component_path, e))?;

    let serialized_component = match component_type {
        ComponentType::EdgeFunction => match component_wit_version {
            "1.0.0" => context.serialize_edge_function_1_0_0(component_path)?,
            _ => anyhow::bail!("Invalid WIT version: {}", component_wit_version),
        },
        _ => anyhow::bail!(
            "Serialization is only supported for Edge Function components at the moment"
        ),
    };

    let serialized_path = format!("{}.serialized", component_path);
    match std::fs::write(&serialized_path, serialized_component) {
        Ok(_) => {
            tracing::info!("Component serialized successfully to {}", serialized_path);
        }
        Err(err) => {
            tracing::error!("Failed to write serialized component to file: {}", err);
            return Err(anyhow::anyhow!(
                "Failed to write serialized component to file: {}",
                err
            ));
        }
    }

    Ok(())
}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use anyhow::Context;

    use crate::components::manifest::{self, Manifest};

    let (component_path, component_type, component_wit_version) = match (
        _opts.filename,
        _opts.component_type,
        _opts.wit_version,
    ) {
        (Some(filename), Some(component_type), Some(version)) => match component_type.as_str() {
            "data-collection" => (filename, ComponentType::DataCollection, version),
            "edge-function" => (filename, ComponentType::EdgeFunction, version),
            _ => anyhow::bail!(
                "Invalid component type: {}, expected 'data-collection' or 'edge-function'",
                component_type
            ),
        },
        _ => {
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
            (
                component_path.to_string(),
                match manifest.component.category {
                    edgee_api_client::types::ComponentCreateInputCategory::DataCollection => {
                        ComponentType::DataCollection
                    }
                    edgee_api_client::types::ComponentCreateInputCategory::EdgeFunction => {
                        ComponentType::EdgeFunction
                    }
                    _ => anyhow::bail!(
                        "Invalid component type: {}, expected 'data-collection'",
                        manifest.component.category
                    ),
                },
                manifest.component.wit_version,
            )
        }
    };

    serialize_component(
        component_type,
        component_path.as_str(),
        component_wit_version.as_str(),
    )
    .await?;

    Ok(())
}
