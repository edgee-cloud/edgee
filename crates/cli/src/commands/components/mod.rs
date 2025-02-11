setup_commands! {
    /// Init a new component manifest in the current directory
    Init(init),
    /// Create component in a new directory with sample code
    New(new),

    /// Compile the component in the current directory into WASM
    Build(build),

    /// Pull a component from the Edgee Component Registry
    Pull(pull),
    /// Push a component to the Edgee Component Registry
    Push(push),
    /// List components you've previously pulled
    List(list),
    /// Check if the local WASM component file is valid
    Check(check),
    /// Run the component in the current folder with sample events
    Test(test)
}

pub type Options = Command;

pub async fn run(command: Command) -> anyhow::Result<()> {
    crate::logger::init_cli();
    command.run().await
}
