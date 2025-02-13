use crate::components::manifest::Manifest;
use edgee_api_client::types as api_types;

#[derive(Debug, clap::Parser)]
pub struct Options {
    /// The organization name used to create or update your component
    ///
    /// Defaults to the user "self" org
    pub organization: Option<String>,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    use inquire::{Confirm, Editor, Select};

    use edgee_api_client::{auth::Credentials, ResultExt};

    use crate::components::manifest;

    let creds = Credentials::load()?;
    creds.check_api_token()?;

    let Some(manifest_path) = manifest::find_manifest_path() else {
        anyhow::bail!("Edgee Manifest not found. Please run `edgee component new` and start from a template or `edgee component init` to create a new empty manifest in this folder.");
    };
    let manifest = Manifest::load(&manifest_path)?;

    let client = edgee_api_client::new().credentials(&creds).connect();

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
            let confirm = Confirm::new(&format!(
                "Component `{}/{}` does not exists, do you want to create it?",
                organization.slug, manifest.package.name,
            ))
            .with_default(true)
            .prompt()?;
            if !confirm {
                return Ok(());
            }

            let public = Select::new(
                "Would you like to make this component public or private?",
                vec!["private", "public"],
            )
            .prompt()?;

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
                        )
                        .public(public == "public"),
                )
                .send()
                .await
                .api_context("Could not create component")?;
            tracing::info!(
                "Component `{}/{}` created successfully!",
                organization.slug,
                manifest.package.name
            );
        }
        Ok(_) | Err(_) => {}
    }

    let changelog =
        Editor::new("Please describe the changes from the previous version").prompt_skippable()?;

    let confirm = Confirm::new(&format!(
        "Please confirm to push the component `{}/{}`:",
        organization.slug, manifest.package.name,
    ))
    .with_default(true)
    .prompt()?;
    if !confirm {
        return Ok(());
    }

    tracing::info!("Uploading WASM file...");
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
                .dynamic_fields(convert_manifest_config_fields(&manifest))
                .changelog(changelog),
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
        .settings
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
