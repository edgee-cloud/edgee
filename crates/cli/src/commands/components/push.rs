use std::io::Read;

use anyhow::Result;
use colored::Colorize;
use edgee_api_client::{auth::Config, ErrorExt, ResultExt};
use inquire::{Confirm, Editor, Select};

use edgee_api_client::{types as api_types, Client};

use crate::components::manifest::Manifest;

setup_command! {
    /// The organization name used to create or update your component
    ///
    /// Defaults to the user "self" org
    organization: Option<String>,

    /// Will use the given login profile
    #[arg(short, long, id = "PROFILE", env = "EDGEE_API_PROFILE")]
    profile: Option<String>,

    /// Will push the component as public
    #[arg(long, conflicts_with = "private")]
    public: bool,

    /// Will push the component as private
    #[arg(long, conflicts_with = "public")]
    private: bool,

    /// Will be used as version changelog (with no inline editor)
    #[arg(long)]
    changelog: Option<String>,

    /// Run this command in non-interactive mode (with no confirmation prompts)
    #[arg(long = "yes")]
    noconfirm: bool,
}

pub async fn run(opts: Options) -> Result<()> {
    use crate::components::manifest;

    let config = Config::load()?;

    let creds = match config.get(&opts.profile) {
        Some(creds) => creds,
        None => {
            match opts.profile {
                None => {
                    anyhow::bail!("No API token configured");
                }
                Some(profile) => {
                    anyhow::bail!("No API token configured for profile '{}'", profile);
                }
            };
        }
    };
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
        if opts.noconfirm {
            anyhow::bail!("No WASM file was found.");
        }

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
        match manifest.component.category {
            api_types::ComponentCreateInputCategory::DataCollection => {
                super::check::ComponentType::DataCollection
            }
            api_types::ComponentCreateInputCategory::EdgeFunction => {
                super::check::ComponentType::EdgeFunction
            }
            _ => anyhow::bail!(
                "Invalid component type: {}, expected 'data-collection'",
                manifest.component.category
            ),
        },
        output_path.to_str().unwrap(),
        &manifest.component.wit_version,
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
            let confirm = opts.noconfirm
                || Confirm::new("Confirm new component creation?")
                    .with_default(true)
                    .prompt()?;

            if !confirm {
                return Ok(());
            }

            let component =
                create_component(&client, &opts, &manifest, &organization, &component_slug).await?;

            (false, component)
        }
        Ok(res) => {
            tracing::info!(
                "Component {} found!",
                match &res.latest_version {
                    Some(version) => {
                        format!("{}/{}@{}", organization.slug, component_slug, version).green()
                    }
                    None => format!("{}/{}", organization.slug, component_slug).green(),
                }
            );
            (true, res.into_inner())
        }
        Err(err) => anyhow::bail!("Error contacting API: {}", err.into_message()),
    };

    // Check if we need to push a new version as well
    let create_version = if !component.versions.contains_key(&manifest.component.version) {
        let changelog = match opts.changelog {
            Some(ref changelog) => Some(changelog.clone()),
            // Set no changelog if none provided in non-interactive mode
            None if opts.noconfirm => None,
            None => Editor::new("Describe the new version changelog (optional)")
                .with_help_message(
                    "Type (e) to open the default editor. Use the EDITOR env variable to change it.",
                )
                .prompt_skippable()?,
        };

        Some(
            push_version()
                .client(&client)
                .manifest(&manifest)
                .organization(&organization)
                .component_slug(&component_slug)
                .maybe_changelog(changelog),
        )
    } else {
        tracing::info!(
            "{} already exists in the registry. Only updating component metadata.",
            format!(
                "{}/{}@{}",
                organization.slug, component_slug, manifest.component.version,
            )
            .green(),
        );
        None
    };

    let confirm = opts.noconfirm
        || Confirm::new(&format!(
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

    if do_update {
        update_component(
            &client,
            &opts,
            &manifest,
            &component,
            &component_slug,
            &organization,
        )
        .await?;
    }
    if let Some(push_version) = create_version {
        push_version.call().await?;
    }

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

async fn create_component(
    client: &Client,
    opts: &Options,
    manifest: &Manifest,
    organization: &api_types::Organization,
    component_slug: &str,
) -> Result<api_types::Component> {
    let public = match (opts.public, opts.private) {
        (true, false) => true,
        (false, true) => false,
        // Set component as private by default if run in non-interactive mode
        _ if opts.noconfirm => false,
        _ => {
            Select::new(
                "Would you like to make this component public or private?",
                vec!["private", "public"],
            )
            .prompt()?
                == "public"
        }
    };

    let avatar_url = if let Some(path) = &manifest.component.icon_path {
        tracing::info!(
            "Uploading Icon... {}",
            manifest
                .component
                .icon_path
                .as_ref()
                .unwrap_or(&String::new())
        );
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
                .slug(Some(component_slug.to_string()))
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
                .public(public),
        )
        .send()
        .await
        .api_context("Could not create component")?;
    tracing::info!(
        "Component {} created successfully!",
        format!("{}/{}", organization.slug, component_slug).green(),
    );

    Ok(component.into_inner())
}

async fn update_component(
    client: &Client,
    opts: &Options,
    manifest: &Manifest,
    component: &api_types::Component,
    component_slug: &str,
    organization: &api_types::Organization,
) -> Result<()> {
    use inquire::Confirm;

    use edgee_api_client::ResultExt;

    let final_icon_url = if let Some(manifest_icon_path) = &manifest.component.icon_path {
        let manifest_avatar_hash = {
            let mut manifest_avatar_file = std::fs::File::open(manifest_icon_path)?;
            hash_reader(&mut manifest_avatar_file)?
        };

        if let Some(existing_avatar_url) = &component.avatar_url {
            let response = reqwest::get(existing_avatar_url).await?;
            let existing_avatar_data = response.bytes().await?;
            let existing_avatar_hash = hash_reader(&existing_avatar_data[..])?;

            if existing_avatar_hash != manifest_avatar_hash {
                tracing::info!("Detected icon change, uploading new icon...");
                let new_avatar_url = client
                    .upload_file(std::path::Path::new(manifest_icon_path))
                    .await?;
                Some(new_avatar_url)
            } else {
                tracing::info!("Icon has not changed, skipping upload...");
                None
            }
        } else if let Some(path) = &manifest.component.icon_path {
            Some(client.upload_file(std::path::Path::new(path)).await?)
        } else {
            None
        }
    } else {
        None
    };

    let public = match (opts.public, opts.private) {
        (true, false) | (false, true) => {
            let public = opts.public;
            let visibility = if public { "public" } else { "private" };
            let remote_public = component.is_public.unwrap_or(false);

            if public == remote_public {
                tracing::info!("Component is already {visibility}");
            } else {
                tracing::info!("Updating component visibility to {visibility}...");

                if !public {
                    tracing::info!("Only unused components can be made private. If this component is already in use, it will remain public.");
                } else {
                    let confirm = opts.noconfirm || Confirm::new(
                            "Your component will become publicly visible in the registry. Are you sure?",
                        )
                        .with_default(true)
                        .prompt()?;

                    if !confirm {
                        return Ok(());
                    }
                }
            }

            public
        }
        _ => component.is_public.unwrap_or(false),
    };

    client
        .update_component_by_slug()
        .org_slug(&organization.slug)
        .component_slug(component_slug)
        .body(
            api_types::ComponentUpdateParams::builder()
                .name(manifest.component.name.clone())
                .description(manifest.component.description.clone())
                .public(public)
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

    Ok(())
}

#[bon::builder]
async fn push_version(
    client: &Client,
    manifest: &Manifest,
    organization: &api_types::Organization,
    component_slug: &str,
    changelog: Option<String>,
) -> Result<()> {
    use edgee_api_client::ResultExt;

    tracing::info!("Uploading WASM file...");
    let asset_url = client
        .upload_file(&manifest.component.build.output_path)
        .await
        .expect("Could not upload component");

    tracing::info!("Creating new version...");

    client
        .create_component_version_by_slug()
        .org_slug(&organization.slug)
        .component_slug(component_slug)
        .body(
            api_types::ComponentVersionCreateInput::builder()
                .version(&manifest.component.version)
                .wit_version(&manifest.component.wit_version)
                .wasm_url(asset_url)
                .dynamic_fields(convert_manifest_config_fields(manifest))
                .changelog(changelog),
        )
        .send()
        .await
        .api_context("Could not create version")?;

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
            options: field.options.clone(),
            secret: field.secret,
        })
        .collect()
}

fn hash_reader<R: Read>(mut reader: R) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}
