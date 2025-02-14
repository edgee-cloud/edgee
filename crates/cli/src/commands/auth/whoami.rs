use edgee_api_client::ResultExt;

setup_command! {}

pub async fn run(_opts: Options) -> anyhow::Result<()> {
    use edgee_api_client::auth::Credentials;

    let creds = Credentials::load()?;
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

    Ok(())
}
