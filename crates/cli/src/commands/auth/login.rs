use anyhow::Result;

use edgee_api_client::ResultExt;

setup_command! {
    #[arg(short, long, id = "PROFILE")]
    profile: Option<String>,

    #[arg(short, long, id = "URL")]
    url: Option<String>,
}

pub async fn run(opts: Options) -> Result<()> {
    use inquire::{Confirm, Password, PasswordDisplayMode};

    use edgee_api_client::auth::{Config, Credentials};

    let mut config = Config::load()?;
    let creds = config.get(&opts.profile);

    let confirm_overwrite =
        Confirm::new("An API token is already configured, do you want to overwrite it?")
            .with_default(false);
    if creds.is_some() && !confirm_overwrite.prompt()? {
        return Ok(());
    }
    let url = opts.url.unwrap_or("https://api.edgee.app".to_string());

    let confirm_auto_open_browser = Confirm::new("Your default browser will be opening to Edgee's API token creation page. Do you want to continue?")
        .with_default(true);

    if confirm_auto_open_browser.prompt()? {
        open::that("https://www.edgee.cloud/~/me/settings/tokens")?;
    }

    let api_token = Password::new("Enter Edgee API token (press Ctrl+R to toggle input display):")
        .with_help_message("You can create one at https://www.edgee.cloud/~/me/settings/tokens")
        .with_display_mode(PasswordDisplayMode::Masked)
        .with_display_toggle_enabled()
        .without_confirmation()
        .with_validator(inquire::required!("API token cannot be empty"))
        .prompt()?;

    let creds = Credentials {
        api_token,
        url: url,
    };

    let client = edgee_api_client::new().credentials(&creds).connect();
    println!("{}", client.baseurl());
    let user = client
        .get_me()
        .send()
        .await
        .api_context("Could not get user info")?
        .into_inner();
    println!("Logged as {} ({})", user.name, user.email);

    config.set(opts.profile, creds);
    config.save()
}
