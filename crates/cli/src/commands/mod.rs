#[macro_use]
mod macros;

setup_commands! {
    #[command(flatten)]
    Auth(auth),
    #[command(visible_alias = "server")]
    Serve(serve),
}
