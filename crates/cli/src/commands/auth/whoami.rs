setup_command! {}

pub async fn run(_opts: Options) {
    use edgee_components::auth::Credentials;

    let creds = Credentials::load().unwrap();
    if creds.api_token.is_none() {
        eprintln!("Not logged in");
        return;
    }

    let user = creds.fetch_user().await.unwrap();

    println!("Logged in as:");
    println!("  Name:  {}", user.name);
    println!("  Email: {}", user.email);
}
