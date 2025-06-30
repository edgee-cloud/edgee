use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::components::boilerplate::LANGUAGE_OPTIONS;

use super::boilerplate::CATEGORY_OPTIONS;
use super::manifest::Manifest;

#[derive(Debug, Deserialize, Serialize)]
struct Lock {
    version: String,
}

impl Lock {
    const FILENAME: &str = "edgee-lock.json";

    fn load(path: &Path) -> Result<Self> {
        use std::fs::File;

        let file = File::open(path)?;
        serde_json::from_reader(file).map_err(Into::into)
    }

    fn save(&self, path: &Path) -> Result<()> {
        use std::fs::File;

        let file = File::create(path)?;
        serde_json::to_writer(file, self)?;

        Ok(())
    }
}

pub async fn update(manifest: &Manifest, root_dir: &Path) -> Result<()> {
    let wit_path = root_dir.join(".edgee/wit");
    if wit_path.exists() {
        tokio::fs::remove_dir_all(&wit_path).await?;
    }
    tokio::fs::create_dir_all(&wit_path).await?;

    let language_config = manifest.component.language.as_deref().map(|name| {
        LANGUAGE_OPTIONS
            .iter()
            .find(|&config| {
                unicase::eq_ascii(&name, &config.name)
                    || config
                        .alias
                        .iter()
                        .any(|alias| unicase::eq_ascii(&name, alias))
            })
            .expect("Unknown component language")
    });

    let category_config = CATEGORY_OPTIONS
        .iter()
        .find(|&config| config.value == manifest.component.category)
        .expect("should have a valid category");

    // Update deps
    let deps_path = wit_path.join("deps");
    let deps_manifest_path = wit_path.join("deps.toml");
    let deps_lock_path = wit_path.join("deps.lock");

    let deps_manifest = format!(
        "\
edgee = \"https://github.com/edgee-cloud/edgee-{world}-wit/archive/refs/tags/v{wit_world_version}.tar.gz\"
{extra}
",
        world = category_config.wit_world,
        wit_world_version = manifest.component.wit_version,
        extra = language_config
            .map(|config| config.deps_extra)
            .unwrap_or_default(),
    );
    tokio::fs::write(&deps_manifest_path, deps_manifest).await?;

    wit_deps::update_path(&deps_manifest_path, &deps_lock_path, &deps_path).await?;

    // Create WIT world file
    let wit_world = match category_config.wit_world {
        "data-collection" => {
            format!(
                "\
package edgee:native;

world {world} {{
  {extra}

  export edgee:components/{world}{version};
}}
",
                world = category_config.wit_world,
                extra = language_config
                    .map(|config| config.wit_world_extra)
                    .unwrap_or_default(),
                version = match (
                    category_config.wit_world,
                    manifest.component.wit_version.as_str()
                ) {
                    ("data-collection", "1.0.0") => "".to_string(),
                    (_, _) => format!("@{}", manifest.component.wit_version),
                }
            )
        }
        "edge-function" => {
            format!(
                "\
package edgee:native;
world {world} {{
  {extra}
   export wasi:http/incoming-handler@0.2.0;
   import wasi:http/outgoing-handler@0.2.0;
}}
",
                world = category_config.wit_world,
                extra = language_config
                    .map(|config| config.wit_world_extra)
                    .unwrap_or_default(),
            )
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported WIT world: {}",
                category_config.wit_world
            ));
        }
    };

    let wit_world_path = wit_path.join("world.wit");
    let mut wit_world_file = tokio::fs::File::create(&wit_world_path).await?;
    wit_world_file.write_all(wit_world.as_bytes()).await?;
    drop(wit_world_file);

    // Write lock file
    let lockfile = wit_path.join(Lock::FILENAME);
    let lock = Lock {
        version: manifest.component.wit_version.clone(),
    };
    lock.save(&lockfile)?;

    tracing::info!("WIT files updated!");

    Ok(())
}

pub async fn should_update(manifest: &Manifest, root_dir: &Path) -> Result<bool> {
    let wit_path = root_dir.join(".edgee/wit");
    let lockfile = wit_path.join(Lock::FILENAME);

    if !lockfile.exists() {
        tracing::info!(
            "Downloading WIT files (v{})",
            manifest.component.wit_version
        );
        update(manifest, root_dir).await?;
        return Ok(true);
    }

    let lock = Lock::load(&lockfile)?;
    if lock.version != manifest.component.wit_version {
        tracing::info!(
            "Updating WIT files (from v{} to v{})",
            lock.version,
            manifest.component.wit_version
        );
        update(manifest, root_dir).await?;
        return Ok(true);
    }

    Ok(false)
}
