setup_command! {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    // self_update uses synchronous reqwest client so we need to run it in a blocking task
    tokio::task::spawn_blocking(move || {
        use self_update::{backends::github::Update, Status};

        let updater = Update::configure()
            .repo_owner("edgee-cloud")
            .repo_name("edgee")
            .bin_name("edgee")
            .current_version(self_update::cargo_crate_version!())
            .show_download_progress(true)
            .build()?;

        match updater.update()? {
            Status::Updated(version) => println!("Updated to {version}"),
            Status::UpToDate(version) => println!("already up-to-date ({version})"),
        }

        Ok(())
    })
    .await?
}
