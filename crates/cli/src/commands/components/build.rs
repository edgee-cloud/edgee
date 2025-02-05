#[derive(Debug, clap::Parser)]
pub struct Options {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use std::process::Command;

    use crate::components::manifest::{self, Manifest};

    let Some(manifest_path) = manifest::find_manifest_path() else {
        anyhow::bail!("Manifest not found");
    };
    let manifest = Manifest::load(&manifest_path).map_err(|err| anyhow::anyhow!(err))?;

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(manifest.package.build.command);
    let _status = cmd.status()?;

    Ok(())
}
