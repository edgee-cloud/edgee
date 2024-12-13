#[macro_use]
mod macros;

setup_commands! {
    Auth(auth),
    #[command(visible_alias = "server")]
    Serve(serve),
}
