use crate::components::manifest::Manifest;
use edgee_api_client::types as api_types;

#[derive(Debug, clap::Parser)]
pub struct Options {
    /// Which organization to create the component into if not existing already.
    ///
    /// Defaults to the user "self" org
    pub organization: Option<String>,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    use edgee_api_client::{auth::Credentials, ResultExt};

    use crate::components::manifest;

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
        .get_component_by_slug()
        .org_slug(&organization.slug)
        .component_slug(&manifest.package.name)
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
                    api_types::ComponentCreateInput::builder()
                        .organization_id(organization.id.clone())
                        .name(&manifest.package.name)
                        .description(manifest.package.description.clone())
                        .category(manifest.package.category)
                        .subcategory(manifest.package.subcategory)
                        .documentation_link(
                            manifest
                                .package
                                .documentation
                                .as_ref()
                                .map(|url| url.to_string()),
                        )
                        .repo_link(
                            manifest
                                .package
                                .repository
                                .as_ref()
                                .map(|url| url.to_string()),
                        ),
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
        .create_component_version_by_slug()
        .org_slug(organization.slug)
        .component_slug(&manifest.package.name)
        .body(
            api_types::ComponentVersionCreateInput::builder()
                .version(&manifest.package.version)
                .wit_world_version(&manifest.package.wit_world_version)
                .wasm_url(asset_url)
                .dynamic_fields(convert_manifest_config_fields(&manifest)),
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

fn convert_manifest_config_fields(manifest: &Manifest) -> Vec<api_types::ConfigurationField> {
    manifest
        .package
        .config_fields
        .iter()
        .map(|(name, field)| api_types::ConfigurationField {
            name: name.clone(),
            title: field.title.clone(),
            type_: field.type_,
            required: field.required,
            description: field.description.clone(),
        })
        .collect()
}
