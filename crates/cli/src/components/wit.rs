use std::path::Path;

use anyhow::{Context, Result};
use async_compression::futures::bufread::GzipDecoder;
use async_tar::Archive;
use futures::{io::BufReader, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use super::boilerplate::CATEGORY_OPTIONS;
use super::manifest::Manifest;

#[derive(Debug, Deserialize, Serialize)]
struct Lock {
    version: String,
}

impl Lock {
    const FILENAME: &str = "lock.json";

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
    let wit_tarball_url = format!(
        "https://github.com/edgee-cloud/edgee-wit/archive/refs/tags/v{}.tar.gz",
        manifest.component.wit_world_version,
    );

    let res = reqwest::get(wit_tarball_url)
        .await
        .context("Could not fetch Edgee WIT files")?
        .error_for_status()
        .context("Fetching Edgee WIT files resulted in error")?;

    let tarball = res
        .bytes_stream()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        .into_async_read();
    let tarball = GzipDecoder::new(BufReader::new(tarball));

    let archive = Archive::new(tarball);
    let mut entries = archive
        .entries()?
        .filter_map(|entry| std::future::ready(entry.ok()))
        .filter(|entry| std::future::ready(entry.path_bytes().ends_with(b".wit")));

    let wit_path = root_dir.join(".edgee/wit");
    if wit_path.exists() {
        tokio::fs::remove_dir_all(&wit_path).await?;
    }

    while let Some(mut entry) = entries.next().await {
        let Ok(entry_path) = entry.path() else {
            continue;
        };
        let entry_path = {
            let mut components = entry_path.as_ref().components();
            // Remove `edgee-wit-X.Y.Z/` prefix from GitHub tarball
            components.next();
            components.as_path()
        };
        if !entry_path.starts_with("wit") {
            continue;
        }
        let entry_path = entry_path.strip_prefix("wit")?;

        let path = wit_path.join(entry_path);
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        entry.unpack(path).await?;
    }

    // Create WIT world file
    let category_config = CATEGORY_OPTIONS
        .iter()
        .find(|&config| config.value == manifest.component.category)
        .expect("should have a valid category");
    let wit_world_path = wit_path.join("world.wit");
    let mut wit_world_file = tokio::fs::File::create(&wit_world_path).await?;
    wit_world_file.write_all(category_config.wit_world).await?;
    drop(wit_world_file);

    let lockfile = wit_path.join(Lock::FILENAME);
    let lock = Lock {
        version: manifest.component.wit_world_version.clone(),
    };
    lock.save(&lockfile)?;

    tracing::info!("Edgee WIT files has been updated");

    Ok(())
}

pub async fn should_update(manifest: &Manifest, root_dir: &Path) -> Result<()> {
    let wit_path = root_dir.join(".edgee/wit");
    let lockfile = wit_path.join(Lock::FILENAME);

    if !lockfile.exists() {
        tracing::info!("Edgee WIT files are missing, downloading them...");
        return update(manifest, root_dir).await;
    }

    let lock = Lock::load(&lockfile)?;
    if lock.version != manifest.component.wit_world_version {
        tracing::info!("Edgee WIT files are out of date, updating them...");
        return update(manifest, root_dir).await;
    }

    Ok(())
}
