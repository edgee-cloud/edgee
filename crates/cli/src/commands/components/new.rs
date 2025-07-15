use colored::Colorize;
use inquire::{Select, Text};
use reqwest::Client;
use std::fs::{create_dir_all, File};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::read::ZipArchive;

use crate::components::boilerplate::{
    CategoryConfig, LanguageConfig, CATEGORY_OPTIONS, LANGUAGE_OPTIONS,
};
use crate::components::manifest::Manifest;

setup_command! {
    /// Will be used as the local folder name
    #[clap(long, short)]
    name: Option<String>,

    /// One of the supported languages (c, c#, go, js, python, rust, typescript)
    #[clap(long, short)]
    language: Option<String>,

    /// One of the supported categories (data-collection, consent-management)
    #[clap(long, short)]
    category: Option<String>,
}

fn prompt_for_language() -> LanguageConfig {
    Select::new("Select a programming language:", LANGUAGE_OPTIONS.to_vec())
        .prompt()
        .expect("Failed to prompt for language")
}

fn prompt_for_category() -> CategoryConfig {
    Select::new("Select the main category:", CATEGORY_OPTIONS.to_vec())
        .prompt()
        .expect("Failed to prompt for category")
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    let component_name = match opts.name {
        Some(name) => name,
        None => Text::new("Enter the component name:")
            .with_validator(inquire::required!("Component name cannot be empty"))
            .with_validator(inquire::min_length!(
                3,
                "Component name must be at least 3 characters"
            ))
            .prompt()?,
    };
    let component_language = match opts
        .language
        .as_deref()
        .filter(|language| !language.is_empty())
    {
        Some(language) => LANGUAGE_OPTIONS
            .iter()
            .find(|l| l.alias.contains(&language.to_lowercase().as_str()))
            .cloned()
            .unwrap_or_else(|| {
                tracing::info!(
                    "Language '{}' is not available. Please select from the list:",
                    language
                );
                prompt_for_language()
            }),
        None => prompt_for_language(),
    };

    let component_category = match opts
        .category
        .as_deref()
        .filter(|category| !category.is_empty())
    {
        Some(category) => CATEGORY_OPTIONS
            .iter()
            .find(|c| c.wit_world.to_lowercase() == category.to_lowercase())
            .cloned()
            .unwrap_or_else(|| {
                tracing::info!(
                    "Category '{}' is not available. Please select from the list:",
                    category
                );
                prompt_for_category()
            }),
        None => prompt_for_category(),
    };

    let component_path = Path::new(&component_name);
    if component_path.exists() {
        anyhow::bail!("A component with this name already exists in this directory");
    }

    let url = format!(
        "{}{}",
        component_language.repo_url, "/archive/refs/heads/main.zip"
    );

    tracing::info!("Downloading sample code for {}...", component_language.name);
    let response = Client::new().get(url).send().await?.bytes().await?;
    let reader = Cursor::new(response);
    let mut archive = ZipArchive::new(reader)?;

    tracing::info!("Extracting code...");
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let path = file.name();

        // split into <repo>/<wit-world>/<rest of file>
        let parts: Vec<&str> = path.splitn(3, '/').collect();
        if parts.len() < 3 {
            continue;
        }

        // only get the wanted world
        if parts[1] != component_category.wit_world {
            continue;
        }

        let output_path = component_path.join(parts[2]);
        if file.is_dir() {
            create_dir_all(&output_path)?;
        } else {
            if let Some(parent) = output_path.parent() {
                create_dir_all(parent)?;
            }
            let mut outfile = File::create(&output_path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            outfile.write_all(&buffer)?;
        }
    }

    let manifest_path = component_path.join(Manifest::FILENAME);
    let manifest = Manifest::load(&manifest_path)?;
    tracing::info!(
        "Downloading WIT files v{}...",
        manifest.component.wit_version
    );
    crate::components::wit::update(&manifest, component_path).await?;

    tracing::info!(
        "New project {} is ready! Check README for dependencies.",
        component_name.green()
    );
    Ok(())
}
