use edgee_components_runtime::config::EdgeFunctionComponents;

setup_command! {
    #[arg(long = "filename")]
    filename: Option<String>,
    #[arg(long = "component-type")]
    component_type: Option<String>,
    #[arg(long = "wit-version")]
    wit_version: Option<String>,
}

fn validate_component_and_wit_version(
    component_type: &str,
    wit_version: &str,
) -> anyhow::Result<()> {
    match component_type {
        "data-collection" => match wit_version {
            "1.0.0" | "1.0.1" => Ok(()),
            _ => anyhow::bail!("Invalid WIT version: {} for data-collection", wit_version),
        },
        "edge-function" => match wit_version {
            "1.0.0" => Ok(()),
            _ => anyhow::bail!("Invalid WIT version: {} for edge-function", wit_version),
        },
        _ => anyhow::bail!(
            "Invalid component type: {}, expected 'data-collection' or 'edge-function'",
            component_type
        ),
    }
}

fn extract_from_manifest() -> anyhow::Result<(String, String, String)> {
    use crate::components::manifest::{self, Manifest};
    use anyhow::Context;

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

    let component_type = match manifest.component.category {
        edgee_api_client::types::ComponentCreateInputCategory::DataCollection => "data-collection",
        edgee_api_client::types::ComponentCreateInputCategory::EdgeFunction => "edge-function",
        _ => anyhow::bail!(
            "Invalid component type: {}, expected 'data-collection' or 'edge-function'",
            manifest.component.category
        ),
    };

    Ok((
        component_path.to_string(),
        component_type.to_string(),
        manifest.component.wit_version,
    ))
}

fn create_components_config(
    component_type: &str,
    component_path: &str,
    component_wit_version: &str,
) -> anyhow::Result<edgee_components_runtime::config::ComponentsConfiguration> {
    use edgee_components_runtime::config::{ComponentsConfiguration, DataCollectionComponents};

    match component_type {
        "data-collection" => {
            let wit_version = match component_wit_version {
                "1.0.0" => edgee_components_runtime::data_collection::versions::DataCollectionWitVersion::V1_0_0,
                "1.0.1" => edgee_components_runtime::data_collection::versions::DataCollectionWitVersion::V1_0_1,
                _ => unreachable!("Already validated"),
            };

            Ok(ComponentsConfiguration {
                data_collection: vec![DataCollectionComponents {
                    id: component_path.to_string(),
                    file: component_path.to_string(),
                    wit_version,
                    ..Default::default()
                }],
                ..Default::default()
            })
        }
        "edge-function" => {
            let wit_version = match component_wit_version {
                "1.0.0" => edgee_components_runtime::edge_function::versions::EdgeFunctionWitVersion::V1_0_0,
                _ => unreachable!("Already validated"),
            };

            Ok(ComponentsConfiguration {
                edge_function: vec![EdgeFunctionComponents {
                    id: component_path.to_string(),
                    file: component_path.to_string(),
                    wit_version,
                    ..Default::default()
                }],
                ..Default::default()
            })
        }
        _ => unreachable!("Already validated"),
    }
}

pub async fn serialize_component(
    component_type: &str,
    component_path: &str,
    component_wit_version: &str,
) -> anyhow::Result<()> {
    use edgee_components_runtime::context::ComponentsContext;

    if !std::fs::exists(component_path)? {
        anyhow::bail!(
            "Component {} does not exist. Please run `edgee component build` first",
            component_path
        );
    }

    validate_component_and_wit_version(component_type, component_wit_version)?;
    let config = create_components_config(component_type, component_path, component_wit_version)?;
    let context = ComponentsContext::new(&config)
        .map_err(|e| anyhow::anyhow!("Invalid component {}: {}", component_path, e))?;

    let serialized_component =
        context.serialize_component(component_path, component_type, component_wit_version)?;

    let serialized_path = format!("{component_path}.serialized");
    std::fs::write(&serialized_path, serialized_component)
        .map_err(|err| anyhow::anyhow!("Failed to write serialized component to file: {}", err))?;

    tracing::info!("Component serialized successfully to {}", serialized_path);
    Ok(())
}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    let (component_path, component_type, component_wit_version) =
        match (_opts.filename, _opts.component_type, _opts.wit_version) {
            (Some(filename), Some(component_type), Some(version)) => {
                (filename, component_type, version)
            }
            _ => extract_from_manifest()?,
        };

    serialize_component(&component_type, &component_path, &component_wit_version).await
}
