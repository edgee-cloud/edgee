setup_command! {}

pub async fn run(_opts: Options) {
    use inquire::{Confirm, Password, PasswordDisplayMode};

    use edgee_components::auth::Credentials;

    let mut creds = Credentials::load().unwrap();

    let confirm_overwrite =
        Confirm::new("An API token is already present, do you want to overwrite it?")
            .with_default(false);
    if creds.api_token.is_some() && !confirm_overwrite.prompt().unwrap() {
        return;
    }

    let api_token = Password::new("Enter Edgee API token (press Ctrl+R to toggle input display):")
        .with_help_message("You can create one at https://www.edgee.cloud/me/settings/tokens")
        .with_display_mode(PasswordDisplayMode::Masked)
        .with_display_toggle_enabled()
        .without_confirmation()
        .with_validator(inquire::required!("API token cannot be empty"))
        .prompt()
        .unwrap();
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
