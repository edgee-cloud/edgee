use colored::Colorize;
use url::Url;

use crate::components::{
    boilerplate::{CATEGORY_OPTIONS, LANGUAGE_OPTIONS, SUBCATEGORY_OPTIONS},
    manifest::{self, Build, Component, Manifest, Setting},
};

setup_command! {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use inquire::{Select, Text};
    if manifest::find_manifest_path().is_some() {
        anyhow::bail!("Manifest already exists");
    }

    let component_name = Text::new("Enter the component name:")
        .with_validator(inquire::required!("Component name cannot be empty"))
        .with_validator(inquire::min_length!(
            3,
            "Component name must be at least 3 characters"
        ))
        .prompt()?;

    let component_language =
        Select::new("Select a programming language:", LANGUAGE_OPTIONS.to_vec()).prompt()?;
    let component_category = if CATEGORY_OPTIONS.len() == 1 {
        CATEGORY_OPTIONS[0].clone() // there is only 1 element
    } else {
        Select::new(
            "Select the main category:",
            CATEGORY_OPTIONS.to_vec(), // Pas besoin de `.to_vec()`, on passe une slice
        )
        .prompt()?
    };

    let component_subcategory =
        Select::new("Select a subcategory:", SUBCATEGORY_OPTIONS.to_vec()).prompt()?;

    tracing::info!(
        "Initiating component {} in {}",
        component_name.green(),
        component_language.name.green(),
    );

    let project_dir = std::env::current_dir()?;

    let manifest = Manifest {
        manifest_version: manifest::Manifest::VERSION,
        component: Component {
            name: component_name.clone(),
            slug: Some(slug::slugify(&component_name)),
            version: "0.1.0".to_string(),
            wit_version: "0.5.0".to_string(),
            category: component_category.value,
            subcategory: component_subcategory.value,
            description: Some("Description of\nthe component".to_string()),
            documentation: Some(Url::parse("https://www.edgee.cloud/docs/introduction")?),
            repository: Some(Url::parse("https://www.github.com/edgee-cloud/edgee")?),
            settings: indexmap::indexmap! {
                "example".to_string() => Setting {
                    description: Some("Here is a string".to_string()),
                    options: None,
                    required: true,
                    title: "ExampleConfigField".to_string(),
                    type_: edgee_api_client::types::ConfigurationFieldType::String,
                    secret: false,
                },
            },
            build: Build {
                command: component_language.default_build_command.to_string(),
                output_path: std::path::PathBuf::from(""),
            },
            icon_path: None,
            language: Some(component_language.name.to_string()),
        },
    };
    manifest.save(&project_dir)?;

    tracing::info!(
        "Downloading WIT files v{}...",
        manifest.component.wit_version
    );
    crate::components::wit::update(&manifest, &project_dir).await?;

    Ok(())
}
