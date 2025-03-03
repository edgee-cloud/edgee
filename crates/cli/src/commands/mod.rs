#[macro_use]
mod macros;

setup_commands! {
    #[command(flatten)]
    Auth(auth),
    /// Components management commands
    #[command(subcommand, visible_alias = "component")]
    Components(components),
    /// Run the Edgee server
    #[command(visible_alias = "server")]
    Serve(serve),
    [cfg(not(feature = "no-self-update"))]
    /// Update the Edgee executable
    SelfUpdate(update),
    /// Print auto-completion script for your shell init file
    GenerateShellCompletion(completion),
}
