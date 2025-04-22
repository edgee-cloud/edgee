use std::path::Path;

use crate::components::manifest::Manifest;

setup_command! {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use crate::components::manifest;

    let Some(manifest_path) = manifest::find_manifest_path() else {
        anyhow::bail!("Edgee Manifest not found. Please run `edgee component new` and start from a template or `edgee component init` to create a new empty manifest in this folder.");
    };
    let root_dir = manifest_path.parent().expect("project root directory");
    let manifest = Manifest::load(&manifest_path).map_err(|err| anyhow::anyhow!(err))?;

    do_build(&manifest, root_dir).await
}

pub async fn do_build(manifest: &Manifest, root_dir: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    crate::components::wit::should_update(manifest, root_dir).await?;

    tracing::info!("Running: {}", manifest.component.build.command);
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&manifest.component.build.command);
    let status = cmd.status()?;

    if status.success() {
        tracing::info!("Build successful!");
        Ok(())
    } else {
        tracing::error!("Build errored!");
        Err(anyhow::anyhow!("Build failed with status code: {}", status))
    }
}
