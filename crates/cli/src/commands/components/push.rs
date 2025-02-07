#[derive(Debug, clap::Parser)]
pub struct Options {
    /// Which organization to create the component into if not existing already.
    ///
    /// Defaults to the user "self" org
    pub organization: Option<String>,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    use edgee_api_client::{auth::Credentials, types, ResultExt};

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

    let organization = match opts.organization {
        Some(ref organization) => client
            .get_organization()
            .id(organization)
            .send()
            .await
            .api_with_context(|| format!("Could not get organization `{organization}`"))?
            .into_inner(),
        None => client
            .get_my_organization()
            .send()
            .await
            .api_context("Could not get user organization")?
            .into_inner(),
    };

    match client
        .get_component()
        .id(&manifest.package.name)
        .send()
        .await
    {
        Err(edgee_api_client::Error::ErrorResponse(err))
            if err.error.type_
                == edgee_api_client::types::ErrorResponseErrorType::NotFoundError =>
        {
            tracing::info!("Component does not exist, creating...");
            client
                .create_component()
                .body(
                    types::ComponentCreateInput::builder()
                        .organization_id(organization.id)
                        .name(&manifest.package.name)
                        .description(manifest.package.description)
                        .category(manifest.package.category)
                        .subcategory(manifest.package.subcategory)
                        .documentation_link(
                            manifest.package.documentation.map(|url| url.to_string()),
                        )
                        .repo_link(manifest.package.repository.map(|url| url.to_string())),
                )
                .send()
                .await
                .api_context("Could not create component")?;
        }
        Ok(_) | Err(_) => {}
    }

    tracing::info!("Uploading output artifact...");
    let asset_url = client
        .upload_file(&manifest.package.build.output_path)
        .await
        .expect("Could not upload component");

    tracing::info!("Creating component version...");
    client
        .create_component_version()
        .id(&manifest.package.name)
        .body(
            types::ComponentVersionCreateInput::builder()
                .version(&manifest.package.version)
                .wit_world_version(&manifest.package.wit_world_version)
                .wasm_url(asset_url),
        )
        .send()
        .await
        .api_context("Could not create version")?;

    tracing::info!(
        "{} {} pushed successfully!",
        manifest.package.name,
        manifest.package.version
    );

    Ok(())
}
