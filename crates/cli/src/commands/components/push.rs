use colored::Colorize;
use std::io::Read;

use edgee_api_client::types as api_types;
use reqwest::get;

use crate::components::manifest::Manifest;

#[derive(Debug, clap::Parser)]
pub struct Options {
    /// The organization name used to create or update your component
    ///
    /// Defaults to the user "self" org
    pub organization: Option<String>,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    use inquire::{Confirm, Editor, Select};

    use edgee_api_client::{auth::Credentials, ErrorExt, ResultExt};

    use crate::components::manifest;

    let creds = Credentials::load()?;
    creds.check_api_token()?;

    let Some(manifest_path) = manifest::find_manifest_path() else {
        anyhow::bail!("Edgee Manifest not found. Please run `edgee component new` and start from a template or `edgee component init` to create a new empty manifest in this folder.");
    };
    let root_dir = manifest_path.parent().expect("project root dir");
    let manifest = Manifest::load(&manifest_path)?;

    // check if the output file exists
    // if not, ask if the user wants to build it
    let output_path = &manifest.component.build.output_path;
    if !output_path.exists() {
        let confirm = Confirm::new(
            "No WASM file was found. Would you like to run `edgee components build` first?",
        )
        .with_default(true)
        .prompt()?;
        if !confirm {
            return Ok(());
        }

        super::build::do_build(&manifest, root_dir).await?;
    }

    // check if the output file is a valid Data Collection component
    match super::check::check_component(
        super::check::ComponentType::DataCollection,
        output_path.to_str().unwrap(),
    )
    .await
    {
        Ok(_) => {}
        Err(_) => {
            anyhow::bail!(format!(
                "File {} is not a valid Data Collection component. Run `edgee component check` for more information.",
                output_path.display(),
            ));
        }
    }

    if manifest.component.name.clone().len() < 3 {
        anyhow::bail!("Component name must be at least 3 characters");
    }

    let client = edgee_api_client::new().credentials(&creds).connect();

    let organization = match opts.organization {
        Some(ref organization) => client
            .get_organization()
            .id(organization)
            .send()
            .await
            .api_with_context(|| format!("Could not get organization {}", organization.green()))?
            .into_inner(),
        None => client
            .get_my_organization()
            .send()
            .await
            .api_context("Could not get user organization")?
            .into_inner(),
    };

    let component_slug = match manifest.component.slug {
        Some(ref slug) => slug.clone(),
        None => slug::slugify(&manifest.component.name),
    };

    let (do_update, component) = match client
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
            tracing::info!(
                "Component {} does not exist yet!",
                format!("{}/{}", organization.slug, &component_slug).green(),
            );
            let confirm = Confirm::new("Confirm new component creation?")
                .with_default(true)
                .prompt()?;

            if !confirm {
                return Ok(());
            }

            let public_or_private = Select::new(
                "Would you like to make this component public or private?",
                vec!["private", "public"],
            )
            .prompt()?;

            let avatar_url = if let Some(path) = &manifest.component.icon_path {
                tracing::info!("Uploading Icon... {:?}", manifest.component.icon_path);
                Some(client.upload_file(std::path::Path::new(path)).await?)
            } else {
                None
            };

            let component = client
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
                        .avatar_url(avatar_url)
                        .public(public_or_private == "public"),
                )
                .send()
                .await
                .api_context("Could not create component")?;
            tracing::info!(
                "Component {} created successfully!",
                format!("{}/{}", organization.slug, component_slug).green(),
            );

            (false, component.into_inner())
        }
        Ok(res) => {
            tracing::info!(
                "Component {} found!",
                format!("{}/{}", organization.slug, &component_slug).green(),
            );
            (true, res.into_inner())
        }
        Err(err) => anyhow::bail!("Error contacting API: {}", err.into_message()),
    };

    // Check if version already exists
    if component.versions.contains_key(&manifest.component.version) {
        anyhow::bail!(
            "{} already exists in the registry.\nDid you forget to update the manifest?",
            format!(
                "{}/{}@{}",
                organization.slug, component_slug, manifest.component.version,
            )
            .green(),
        );
    }

    let changelog = Editor::new("Describe the new version changelog (optional)")
        .with_help_message(
            "Type (e) to open the default editor. Use the EDITOR env variable to change it.",
        )
        .prompt_skippable()?;

    let confirm = Confirm::new(&format!(
        "Ready to push {}. Confirm?",
        format!(
            "{}/{}@{}",
            organization.slug,
            component_slug,
            manifest.component.version.clone()
        )
        .green(),
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

    if do_update {
        let mut final_icon_url = None;

        if let Some(manifest_icon_path) = &manifest.component.icon_path {
            let manifest_avatar_hash = {
                let mut manifest_avatar_file = std::fs::File::open(manifest_icon_path)?;
                hash_reader(&mut manifest_avatar_file)?
            };
            if let Some(existing_avatar_url) = &component.avatar_url {
                let response = get(existing_avatar_url).await?;
                let existing_avatar_data = response.bytes().await?;
                let existing_avatar_hash = hash_reader(&existing_avatar_data[..])?;
                if existing_avatar_hash != manifest_avatar_hash {
                    tracing::info!("Detected icon change, uploading new icon...");
                    let new_avatar_url = client
                        .upload_file(std::path::Path::new(manifest_icon_path))
                        .await?;
                    final_icon_url = Some(new_avatar_url);
                } else {
                    tracing::info!("Icon has not changed, skipping upload...");
                }
            } else {
                let icon_url = if let Some(path) = &manifest.component.icon_path {
                    Some(client.upload_file(std::path::Path::new(path)).await?)
                } else {
                    None
                };
                final_icon_url = icon_url;
            }
        }

        client
            .update_component_by_slug()
            .org_slug(&organization.slug)
            .component_slug(&component_slug)
            .body(
                api_types::ComponentUpdateParams::builder()
                    .name(manifest.component.name.clone())
                    .description(manifest.component.description.clone())
                    .public(component.is_public)
                    .documentation_link(
                        manifest
                            .component
                            .documentation
                            .as_ref()
                            .map(|url| url.to_string()),
                    )
                    .avatar_url(final_icon_url)
                    .repo_link(
                        manifest
                            .component
                            .repository
                            .as_ref()
                            .map(|url| url.to_string()),
                    ),
            )
            .send()
            .await
            .api_context("Could not update component infos")?;
        tracing::info!(
            "Component {} updated successfully!",
            format!("{}/{}", organization.slug, component_slug).green(),
        );
    }

    tracing::info!("Creating new version...");

    client
        .create_component_version_by_slug()
        .org_slug(&organization.slug)
        .component_slug(&component_slug)
        .body(
            api_types::ComponentVersionCreateInput::builder()
                .version(&manifest.component.version)
                .wit_world_version(&manifest.component.wit_version)
                .wasm_url(asset_url)
                .dynamic_fields(convert_manifest_config_fields(&manifest))
                .changelog(changelog),
        )
        .send()
        .await
        .api_context("Could not create version")?;

    tracing::info!(
        "{} pushed successfully!",
        format!(
            "{}/{}@{}",
            organization.slug, component_slug, manifest.component.version,
        )
        .green(),
    );
    tracing::info!(
        "URL: {}",
        format!(
            "https://www.edgee.cloud/~/registry/{}/{}",
            organization.slug, component_slug,
        )
        .green(),
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

fn hash_reader<R: Read>(mut reader: R) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}
