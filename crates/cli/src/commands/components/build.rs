use crate::components::manifest::Manifest;

#[derive(Debug, clap::Parser)]
pub struct Options {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use crate::components::manifest;

    let Some(manifest_path) = manifest::find_manifest_path() else {
        anyhow::bail!("Edgee Manifest not found. Please run `edgee component new` and start from a template or `edgee component init` to create a new empty manifest in this folder.");
    };
    let manifest = Manifest::load(&manifest_path).map_err(|err| anyhow::anyhow!(err))?;

    do_build(&manifest).await?;

    Ok(())
}

pub async fn do_build(manifest: &Manifest) -> anyhow::Result<()> {
    use std::process::Command;

    tracing::info!("Running: {}", manifest.component.build.command);
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&manifest.component.build.command);
    let status = cmd.status()?;

    if status.success() {
        tracing::info!("Build successful!");
    } else {
        tracing::error!("Build errored!");
    }

    Ok(())
}
