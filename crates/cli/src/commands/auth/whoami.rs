setup_command! {}

pub async fn run(_opts: Options) {
    use edgee_api_client::auth::Credentials;

    let creds = Credentials::load().unwrap();
    if creds.api_token.is_none() {
        eprintln!("Not logged in");
        return;
    }

    let client = edgee_api_client::new().credentials(&creds).connect();
    let user = client.get_me().send().await.unwrap();

    println!("Logged in as:");
    println!("  ID:    {}", user.id);
    println!("  Name:  {}", user.name);
    println!("  Email: {}", user.email);
}
