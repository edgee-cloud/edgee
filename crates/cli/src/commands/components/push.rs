use anyhow::Context;

#[derive(Debug, clap::Parser)]
pub struct Options {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use edgee_api_client::{auth::Credentials, types};

    use crate::components::manifest::{self, Manifest};

    let creds = Credentials::load()?;
    creds.check_api_token()?;

    let Some(manifest_path) = manifest::find_manifest_path() else {
        anyhow::bail!("Manifest not found");
    };
    let manifest = Manifest::load(&manifest_path)?;

    let client = edgee_api_client::new()
        .baseurl("https://api.edgee.dev")
        .credentials(&creds)
        .connect();

    let asset_url = client
        .upload_file(&manifest.package.build.output_path)
        .await
        .expect("Could not upload component");

    let _version = client
        .create_component_version()
        .id(manifest.package.name)
        .body(types::ComponentVersionCreateInput {
            changelog: None,
            dynamic_fields: Vec::new(),
            object: None,
            version: manifest.package.version.clone(),
            wit_world_version: manifest.package.wit_world_version.clone(),
            wasm_url: asset_url,
        })
        .send()
        .await
        .context("Could not create a component version")?;

    Ok(())
}
