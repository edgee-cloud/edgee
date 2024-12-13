setup_commands! {
    Login(login),
    Whoami(whoami),
}

setup_command! {
    #[command(subcommand)]
    command: Command,
}

pub async fn run(opts: Options) {
    opts.command.run().await
}
