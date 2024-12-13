setup_command! {}

pub async fn run(_opts: Options) {
    use inquire::{Confirm, Text};

    use edgee_components::auth::Credentials;

    let mut creds = Credentials::load().unwrap();

    let confirm_overwrite =
        Confirm::new("An API token is already present, do you want to overwrite it?")
            .with_default(false);
    if creds.api_token.is_some() && !confirm_overwrite.prompt().unwrap() {
        return;
    }

    let api_token = Text::new("Enter Edgee API token (you can create one at https://www.edgee.cloud/<username>/settings/tokens):").prompt().unwrap();
    creds.api_token.replace(api_token);

    let user = match creds.fetch_user().await {
        Ok(user) => user,
        Err(err) => {
            tracing::error!("{err:?}");
            return;
        }
    };
    println!("Logged as {} ({})", user.name, user.email);

    creds.save().unwrap();
}
