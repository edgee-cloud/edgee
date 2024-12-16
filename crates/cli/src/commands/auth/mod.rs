setup_commands! {
    Login(login),
    Whoami(whoami),
}

pub type Options = Command;

pub async fn run(command: Command) {
    command.run().await
}
