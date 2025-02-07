#[macro_use]
mod macros;

setup_commands! {
    #[command(flatten)]
    Auth(auth),
    /// Components management commands
    #[command(subcommand, alias = "component")]
    Components(components),
    /// Run the Edgee server
    #[command(visible_alias = "server")]
    Serve(serve),
}
