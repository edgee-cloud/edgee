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
}

pub type Options = Command;

pub async fn run(command: Command) -> anyhow::Result<()> {
    crate::logger::init_cli();

    let cmd = command.clone();

    let res = command.run().await;
    let _ = send_telemetry_event(&cmd, &res).await;
    res
}

async fn send_telemetry_event(command: &Command, res: &anyhow::Result<()>) -> anyhow::Result<()> {
    use std::collections::HashMap;

    use crate::telemetry::Event;

    let command_name = match command {
        Command::Build(_) => "build",
        Command::Check(_) => "check",
        Command::Init(_) => "init",
        Command::List(_) => "list",
        Command::New(_) => "new",
        Command::Pull(_) => "pull",
        Command::Push(_) => "push",
        Command::Test(_) => "test",
    };

    let mut properties = HashMap::new();
    properties.insert("command".to_string(), format!("components {command_name}"));
    properties.insert(
        "result".to_string(),
        if res.is_ok() {
            "ok".to_string()
        } else {
            "error".to_string()
        },
    );

    Event::builder()
        .name("command")
        .title(format!("components {command_name}"))
        .properties(properties)
        .send()
        .await
}
