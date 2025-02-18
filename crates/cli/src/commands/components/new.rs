use reqwest::Client;
use std::fs::{create_dir_all, File};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::read::ZipArchive;

use crate::components::boilerplate::{LanguageConfig, LANGUAGE_OPTIONS};

#[derive(Debug, clap::Parser)]
pub struct Options {
    #[clap(long, short)]
    pub name: Option<String>,

    #[clap(long, short)]
    pub language: Option<String>,
}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use inquire::{Select, Text};

    let component_name = match _opts.name {
        Some(name) => name,
        None => Text::new("Enter the name of the component:")
            .with_validator(inquire::required!("Component name cannot be empty"))
            .with_validator(inquire::min_length!(
                3,
                "Component name must be at least 3 characters"
            ))
            .prompt()?,
    };

    let prompt_language = || -> LanguageConfig {
        Select::new(
            "Select the language of the component:",
            LANGUAGE_OPTIONS.to_vec(),
        )
        .prompt()
        .expect("Failed to prompt for language")
    };

    let component_language = _opts
        .language
        .as_deref()
        .and_then(|language| {
            LANGUAGE_OPTIONS
                .iter()
                .find(|l| l.alias.contains(&language.to_lowercase().as_str()))
                .cloned()
        })
        .unwrap_or_else(|| {
            tracing::info!(
                "Language '{}' not available. Please select from the list:",
                _opts.language.as_deref().unwrap_or("Unknown")
            );
            prompt_language()
        });

    let component_path = Path::new(&component_name);
    if component_path.exists() {
        anyhow::bail!("A component with this name already exists in this directory");
    }

    let url = format!(
        "{}{}",
        component_language.repo_url, "/archive/refs/heads/main.zip"
    );

    println!("Downloading sample code for {}...", component_language.name);
    let response = Client::new().get(url).send().await?.bytes().await?;
    let reader = Cursor::new(response);
    let mut archive = ZipArchive::new(reader)?;

    println!("Extracting code...");
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let path = file.name();

        // Skip the first-level folder and extract only its contents
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() < 2 {
            continue; // Ignore root-level files or folders
        }

        let output_path = component_path.join(parts[1]);
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
    println!(
        "New project {} setup, check README to install the correct dependencies",
        component_name
    );
    Ok(())
}
