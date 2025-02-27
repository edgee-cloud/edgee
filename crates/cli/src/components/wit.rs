use std::path::Path;

use anyhow::{Context, Result};
use async_compression::futures::bufread::GzipDecoder;
use async_tar::Archive;
use futures::{io::BufReader, StreamExt, TryStreamExt};

use super::manifest::Manifest;

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

    let wit_path = root_dir.join("wit");
    if wit_path.exists() {
        tracing::info!("The existing wit/ directory will be overwritten");
        tokio::fs::remove_dir_all(wit_path).await?;
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

        let path = root_dir.join(entry_path);
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        entry.unpack(path).await?;
    }

    Ok(())
}
