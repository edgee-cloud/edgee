use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use self::models::User;

pub mod models;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Credentials {
    pub api_token: Option<String>,
}

impl Credentials {
    pub fn path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not get user config directory"))?
            .join("edgee");
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir).context("Could not create Edgee config dir")?;
        }

        Ok(config_dir.join("credentials.toml"))
    }

    pub fn load() -> Result<Self> {
        let creds_path = Self::path()?;
        if !creds_path.exists() {
            return Ok(Self::default());
        }

        let content =
            std::fs::read_to_string(creds_path).context("Could not read credentials file")?;
        toml::from_str(&content).context("Could not load credentials file")
    }

    pub fn save(&self) -> Result<()> {
        use std::io::Write;

        let content =
            toml::to_string_pretty(self).context("Could not serialize credentials data")?;

        let creds_path = Self::path()?;

        let mut file = {
            use std::fs::OpenOptions;

            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);

            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;

                // Set credentials file permissions to 0600 (u=rw-,g=,o=)
                // so only the user has access.
                options.mode(0o0600);
            }

            options.open(creds_path)?
        };

        file.write_all(content.as_bytes())
            .context("Could not write credentials data")
    }

    pub async fn fetch_user(&self) -> Result<User> {
        let Some(ref api_token) = self.api_token else {
            anyhow::bail!("No API token provided");
        };

        let client = edgee_api_client::new().api_token(api_token).connect();
        let res = client.get_me().send().await?;

        Ok(res.into_inner().into())
    }
}
