#[macro_use]
mod macros;

setup_commands! {
    #[command(visible_alias = "server")]
    Serve(serve),
}
