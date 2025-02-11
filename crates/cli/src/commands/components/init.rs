use crate::components::{
    boilerplate::{CATEGORY_OPTIONS, LANGUAGE_OPTIONS, SUBCATEGORY_OPTIONS},
    manifest::{self, Build, Manifest, Package},
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
        package: Package {
            name: component_name,
            version: "0.1.0".to_string(),
            wit_world_version: "0.4.0".to_string(),
            category: *component_category.value,
            subcategory: *component_subcategory.value,
            description: None,
            documentation: None,
            repository: None,
            config_fields: Default::default(),
            build: Build {
                command: component_language.default_build_command.to_string(),
                output_path: std::path::PathBuf::from(""),
            },
        },
    }
    .save(std::path::Path::new("./"))?;

    Ok(())
}
