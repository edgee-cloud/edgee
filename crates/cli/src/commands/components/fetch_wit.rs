setup_command! {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use crate::components::manifest::{self, Manifest};

    let Some(manifest_path) = manifest::find_manifest_path() else {
        anyhow::bail!("Edgee Manifest not found. Please run `edgee component new` and start from a template or `edgee component init` to create a new empty manifest in this folder.");
    };
    let root_dir = manifest_path.parent().expect("project root directory");
    let manifest = Manifest::load(&manifest_path).map_err(|err| anyhow::anyhow!(err))?;

    if !crate::components::wit::should_update(&manifest, root_dir).await? {
        tracing::info!("WIT files are up-to-date");
    }

    Ok(())
}
