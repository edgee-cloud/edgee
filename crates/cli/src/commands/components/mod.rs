setup_commands! {
    /// Compile the component in the current directory into Wasm
    Build(build),

    /// Check if the local Wasm file file is valid
    Check(check),

    /// Initialize a new manifest file in the current directory
    Init(init),

    /// List components you've previously pulled (coming soon)
    List(list),

    /// Create component in a new directory with sample code
    New(new),

    /// Pull a component from the Edgee Component Registry (coming soon)
    Pull(pull),

    /// Push a component to the Edgee Component Registry
    Push(push),

    /// Run the component in the current folder with sample events
    Test(test),

    /// Fetch WIT dependencies needed for building
    Wit(fetch_wit),

    /// Serialize component
    /// This serializes the component to a vector of bytes.
    /// This is a low-level operation and is not needed for most users.
    Serialize(serialize),
}

pub type Options = Command;

pub async fn run(command: Command) -> anyhow::Result<()> {
    crate::logger::init_cli();

    command.run().await
}
