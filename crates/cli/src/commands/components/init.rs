use url::Url;

use crate::components::{
    boilerplate::{CATEGORY_OPTIONS, LANGUAGE_OPTIONS, SUBCATEGORY_OPTIONS},
    manifest::{self, Build, Component, Manifest, Setting},
};

#[derive(Debug, clap::Parser)]
pub struct Options {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use inquire::{Select, Text};
    if manifest::find_manifest_path().is_some() {
        anyhow::bail!("Manifest already exists");
    }

    let component_name = Text::new("Enter the name of the component:")
        .with_validator(inquire::required!("Component name cannot be empty"))
        .with_validator(inquire::min_length!(
            3,
            "Component name must be at least 3 characters"
        ))
        .prompt()?;

    let component_language = Select::new(
        "Select the language of the component:",
        LANGUAGE_OPTIONS.to_vec(),
    )
    .prompt()?;
    let component_category = if CATEGORY_OPTIONS.len() == 1 {
        CATEGORY_OPTIONS[0].clone() // Accès direct car on sait qu'il y a un seul élément
    } else {
        Select::new(
            "Select the category of the component:",
            CATEGORY_OPTIONS.to_vec(), // Pas besoin de `.to_vec()`, on passe une slice
        )
        .prompt()?
    };

    let component_subcategory = Select::new(
        "Select the subcategory of the component:",
        SUBCATEGORY_OPTIONS.to_vec(),
    )
    .prompt()?;

    println!(
        "Initiating component {} in {}",
        component_name, component_language.name
    );

    Manifest {
        manifest_version: manifest::MANIFEST_VERSION,
        component: Component {
            name: component_name,
            version: "0.1.0".to_string(),
            wit_world_version: "0.4.0".to_string(),
            category: *component_category.value,
            subcategory: *component_subcategory.value,
            description: Some("Description of the component".to_string()),
            documentation: Some(Url::parse("https://www.edgee.cloud/docs/introduction")?),
            repository: Some(Url::parse("https://www.github.com/edgee-cloud/edgee")?),
            settings: {
                let mut fields = std::collections::HashMap::new();
                fields.insert(
                    "example".to_string(),
                    Setting {
                        description: Some("Here is a\nstring".to_string()),
                        required: true,
                        title: "ExampleConfigField".to_string(),
                        type_: edgee_api_client::types::ConfigurationFieldType::String,
                    },
                );
                fields
            },
            build: Build {
                command: component_language.default_build_command.to_string(),
                output_path: std::path::PathBuf::from(""),
            },
        },
    }
    .save(std::path::Path::new("./"))?;

    Ok(())
}
