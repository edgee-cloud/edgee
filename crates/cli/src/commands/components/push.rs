use crate::components::manifest::Manifest;
use edgee_api_client::types as api_types;
use slug;
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
    let component_slug = slug::slugify(&manifest.component.name);
    match client
        .get_component_by_slug()
        .org_slug(&organization.slug)
        .component_slug(&component_slug)
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
                organization.slug, &component_slug,
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
                        .name(&manifest.component.name)
                        .slug(component_slug.clone())
                        .description(manifest.component.description.clone())
                        .category(manifest.component.category)
                        .subcategory(manifest.component.subcategory)
                        .documentation_link(
                            manifest
                                .component
                                .documentation
                                .as_ref()
                                .map(|url| url.to_string()),
                        )
                        .repo_link(
                            manifest
                                .component
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
                component_slug
            );
        }
        Ok(_) | Err(_) => {}
    }

    let changelog =
        Editor::new("Please describe the changes from the previous version").prompt_skippable()?;

    let confirm = Confirm::new(&format!(
        "Please confirm to push the component `{}/{}`:",
        organization.slug, component_slug,
    ))
    .with_default(true)
    .prompt()?;
    if !confirm {
        return Ok(());
    }

    tracing::info!("Uploading WASM file...");
    let asset_url = client
        .upload_file(&manifest.component.build.output_path)
        .await
        .expect("Could not upload component");

    tracing::info!("Creating component version...");
    client
        .create_component_version_by_slug()
        .org_slug(organization.slug)
        .component_slug(&component_slug)
        .body(
            api_types::ComponentVersionCreateInput::builder()
                .version(&manifest.component.version)
                .wit_world_version(&manifest.component.wit_world_version)
                .wasm_url(asset_url)
                .dynamic_fields(convert_manifest_config_fields(&manifest))
                .changelog(changelog),
        )
        .send()
        .await
        .api_context("Could not create version")?;

    tracing::info!(
        "{} {} pushed successfully!",
        component_slug,
        manifest.component.version,
    );

    Ok(())
}

fn convert_manifest_config_fields(manifest: &Manifest) -> Vec<api_types::ConfigurationField> {
    manifest
        .component
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
