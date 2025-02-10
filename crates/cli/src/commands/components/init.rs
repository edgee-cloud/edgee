use crate::components::{
    boilerplates::LANGUAGE_OPTIONS,
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
        .prompt()?;

    let component_language = Select::new(
        "Select the language of the component:",
        LANGUAGE_OPTIONS.to_vec(),
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
            build: Build {
                command: component_language.default_build_command.to_string(),
                output_path: std::path::PathBuf::from(""),
            },
        },
    }
    .save(std::path::Path::new("./"))?;

    Ok(())
}
