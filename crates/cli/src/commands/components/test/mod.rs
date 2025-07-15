use std::collections::HashMap;

use crate::components::manifest;
use crate::components::manifest::Manifest;

mod test_data_collection_v1_0_0;
mod test_data_collection_v1_0_1;
mod test_edge_function_v1_0_0;

setup_command! { /// Comma-separated key=value pairs for settings
    #[arg(long="settings", value_parser = parse_settings)]
    settings: Option<HashMap<String, String>>,

    /// File containing the settings
    #[arg(long = "settings-file")]
    settings_file: Option<String>,

    /// File containing an array of events to test
    #[arg(long = "events-file")]
    events_file: Option<String>,

    /// Data collection options
    ///
    /// The event type you want to test
    #[arg(long = "event-type", value_parser = ["page", "track", "user"])]
    event_type: Option<String>,

    /// Whether to log the full input event or not (false by default)
    #[arg(long = "display-input", default_value = "false")]
    display_input: bool,

    /// Will print to stdout the cURL command for your EdgeeRequest
    #[arg(long = "curl", default_value = "false")]
    curl: bool,

    /// Will automatically make an HTTP request for your EdgeeRequest
    #[arg(long = "make-http-request", default_value = "false")]
    make_http_request: bool,

    /// Edge function options
    /// The port to run the HTTP server on
    #[arg(long = "port", default_value = "8080")]
    port: u16,

    /// When enabled, the component is automatically rebuilt when the source code changes
    #[arg(long = "watch", default_value = "false")]
    watch: bool,
}

fn parse_settings(settings_str: &str) -> Result<HashMap<String, String>, String> {
    let mut settings_map = HashMap::new();

    for setting in settings_str.split(',') {
        let parts: Vec<&str> = setting.splitn(2, '=').collect();
        if parts.len() == 2 {
            settings_map.insert(parts[0].to_string(), parts[1].to_string());
        } else {
            return Err(format!("Invalid setting: {setting}\nPlease use a comma-separated list of settings such as `-s 'key1=value,key2=value2'`"));
        }
    }

    Ok(settings_map)
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    let manifest_path =
        manifest::find_manifest_path().ok_or_else(|| anyhow::anyhow!("Manifest not found"))?;

    let manifest = Manifest::load(&manifest_path)?;

    let root_dir = manifest_path.parent().expect("project root directory");
    crate::commands::components::build::do_build(&manifest, root_dir).await?;

    match manifest.component.category {
        edgee_api_client::types::ComponentCreateInputCategory::DataCollection => {
            match manifest.component.wit_version.as_str() {
                "1.0.0" => {
                    test_data_collection_v1_0_0::test_data_collection_component_1_0_0(
                        opts, &manifest,
                    )
                    .await?;
                }
                "1.0.1" => {
                    test_data_collection_v1_0_1::test_data_collection_component_1_0_1(
                        opts, &manifest,
                    )
                    .await?;
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported wit version {} for data-collection component",
                        manifest.component.wit_version
                    ));
                }
            }
        }
        edgee_api_client::types::ComponentCreateInputCategory::EdgeFunction => {
            match manifest.component.wit_version.as_str() {
                "1.0.0" => {
                    test_edge_function_v1_0_0::test_edge_function_component(opts, &manifest)
                        .await?;
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported wit version {} for data-collection component",
                        manifest.component.wit_version
                    ));
                }
            }
        }
        _ => anyhow::bail!(
            "Invalid component type: {}, expected 'data-collection'",
            manifest.component.category
        ),
    }

    Ok(())
}
