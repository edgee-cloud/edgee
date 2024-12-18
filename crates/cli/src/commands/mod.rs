#[macro_use]
mod macros;

setup_commands! {
    #[command(flatten)]
    Auth(auth),
    /// Run the Edgee server
    #[command(visible_alias = "server")]
    Serve(serve),
}
