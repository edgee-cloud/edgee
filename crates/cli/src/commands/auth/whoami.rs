setup_command! {}

pub async fn run(_opts: Options) {
    use edgee_components::auth::Credentials;

    let creds = Credentials::load().unwrap();
    let user = creds.fetch_user().await.unwrap();

    println!("Logged in as:");
    println!("  Name:  {}", user.name);
    println!("  Email: {}", user.email);
}
