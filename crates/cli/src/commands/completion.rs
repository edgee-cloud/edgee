use clap_complete::Shell;

setup_command! {
    #[arg(value_enum)]
    shell: Option<Shell>,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    let Some(shell) = opts.shell.or_else(Shell::from_env) else {
        anyhow::bail!("No valid $SHELL found, you need to specify it in the command");
    };

    let mut cli = <crate::Options as clap::CommandFactory>::command();
    let name = cli.get_name().to_owned();

    clap_complete::generate(shell, &mut cli, name, &mut std::io::stdout());

    Ok(())
}
