#[derive(Debug, clap::Parser)]
pub struct Options {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use std::process::Command;

    use crate::components::manifest::{self, Manifest};

    let Some(manifest_path) = manifest::find_manifest_path() else {
        anyhow::bail!("Manifest not found");
    };
    let manifest = Manifest::load(&manifest_path).map_err(|err| anyhow::anyhow!(err))?;

    tracing::info!("Running: {}", manifest.package.build.command);
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(manifest.package.build.command);
    let status = cmd.status()?;

    if status.success() {
        tracing::info!("Build successful!");
    } else {
        tracing::error!("Build errored!");
    }

    Ok(())
}
