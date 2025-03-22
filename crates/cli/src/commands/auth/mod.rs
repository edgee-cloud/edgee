setup_commands! {
    /// Log in to the Edgee Console
    Login(login),
    /// Print currently login informations
    Whoami(whoami),
}

pub type Options = Command;

pub async fn run(command: Command) -> anyhow::Result<()> {
    crate::logger::init_cli();

    let _ = crate::telemetry::setup().inspect_err(|err| tracing::debug!("Telemetry error: {err}"));

    command.run().await
}
