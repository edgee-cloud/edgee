use edgee_api_client::ResultExt;

setup_command! {
    #[arg(short, long, id = "PROFILE", env = "EDGEE_API_PROFILE")]
    profile: Option<String>,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    use edgee_api_client::auth::Config;

    let config = Config::load()?;

    let creds = match config.get(&opts.profile) {
        Some(creds) => creds,
        None => {
            match opts.profile {
                None => {
                    anyhow::bail!("No API token configured");
                }
                Some(profile) => {
                    anyhow::bail!("No API token configured for profile '{}'", profile);
                }
            };
        }
    };

    creds.check_api_token()?;

    let client = edgee_api_client::new().credentials(&creds).connect();
    let user = client
        .get_me()
        .send()
        .await
        .api_context("Could not get user infos")?;

    println!("Logged in as:");
    println!("  ID:    {}", user.id);
    println!("  Name:  {}", user.name);
    println!("  Email: {}", user.email);
    if let Some(url) = &creds.url {
        println!("  Url:   {url}");
    }
    if let Some(profile) = opts.profile {
        println!("  Profile: {profile}");
    }

    Ok(())
}
