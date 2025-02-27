use anyhow::Result;
use clap::Parser;

use crate::components::manifest::Manifest;

#[derive(Debug, Parser)]
enum Command {
    Update,
}

#[derive(Debug, Parser)]
pub struct Options {
    #[command(subcommand)]
    command: Command,
}

pub async fn run(opts: Options) -> Result<()> {
    use crate::components::manifest::find_manifest_path;

    let Some(manifest_path) = find_manifest_path() else {
        anyhow::bail!("Edgee Manifest not found. Please run `edgee component new` and start from a template or `edgee component init` to create a new empty manifest in this folder.");
    };
    let manifest = Manifest::load(&manifest_path)?;

    let root_dir = manifest_path.parent().expect("the project directory");

    match opts.command {
        Command::Update => crate::components::wit::update(&manifest, root_dir).await,
    }
}
