setup_commands! {
    /// Init a new component in the current directory
    Init(init),
    /// Create a new component in a new directory
    New(new),

    /// Build the component
    Build(build),

    /// Pull a component
    Pull(pull),
    /// Push a component
    Push(push),
    /// List currently pulled components
    List(list),
}

pub type Options = Command;

pub async fn run(command: Command) -> anyhow::Result<()> {
    crate::logger::init_cli();
    command.run().await
}
