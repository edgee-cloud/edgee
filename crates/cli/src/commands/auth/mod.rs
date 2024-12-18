setup_commands! {
    /// Log in to the Edgee Console
    Login(login),
    /// Print currently login informations
    Whoami(whoami),
}

pub type Options = Command;

pub async fn run(command: Command) {
    command.run().await
}
