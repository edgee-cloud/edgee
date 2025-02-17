#[derive(Debug, clap::Parser)]
pub struct Options {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    tracing::info!("This command is coming soon!");
    Ok(())
}
