use anyhow::Result;

use edgee_api_client::ResultExt;

setup_command! {}

pub async fn run(_opts: Options) -> Result<()> {
    use inquire::{Confirm, Password, PasswordDisplayMode};

    use edgee_api_client::auth::Credentials;

    let mut creds = Credentials::load()?;

    let confirm_overwrite =
        Confirm::new("An API token is already present, do you want to overwrite it?")
            .with_default(false);
    if creds.api_token.is_some() && !confirm_overwrite.prompt()? {
        return Ok(());
    }

    let api_token = Password::new("Enter Edgee API token (press Ctrl+R to toggle input display):")
        .with_help_message("You can create one at https://www.edgee.cloud/~/me/settings/tokens")
        .with_display_mode(PasswordDisplayMode::Masked)
        .with_display_toggle_enabled()
        .without_confirmation()
        .with_validator(inquire::required!("API token cannot be empty"))
        .prompt()?;
    creds.api_token.replace(api_token);

    let client = edgee_api_client::new().credentials(&creds).connect();

    let user = client
        .get_me()
        .send()
        .await
        .api_context("Could not get user infos")?
        .into_inner();
    println!("Logged as {} ({})", user.name, user.email);

    creds.save()
}
