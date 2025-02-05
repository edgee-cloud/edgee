setup_commands! {
    /// Log in to the Edgee Console
    Login(login),
    /// Print currently login informations
    Whoami(whoami),
}

pub type Options = Command;

pub async fn run(command: Command) -> anyhow::Result<()> {
    crate::logger::init_cli();
    command.run().await
}
