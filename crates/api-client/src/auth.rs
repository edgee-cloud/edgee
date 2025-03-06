use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    connect_builder::{IsUnset, SetApiToken, SetBaseurl, State},
    ConnectBuilder,
};

#[derive(Debug, Deserialize, Default, Serialize, Clone)]
pub struct Config {
    default: Option<Credentials>,
    #[serde(flatten)]
    profiles: std::collections::HashMap<String, Credentials>,
}

#[derive(Debug, Deserialize, Default, Serialize, Clone)]
pub struct Credentials {
    pub api_token: String,
    pub url: String,
}

impl Config {
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

    pub fn get(&self, profile: &Option<String>) -> Option<Credentials> {
        match profile {
            Some(profile) => {
                if let Some(creds) = self.profiles.get(profile) {
                    Some(creds.clone())
                } else {
                    None
                }
            }
            None => self.default.clone(),
        }
    }

    pub fn set(&mut self, profile: Option<String>, creds: Credentials) {
        match profile {
            Some(profile) => {
                self.profiles.insert(profile, creds);
            }
            None => {
                self.default = Some(creds);
            }
        }
    }
}

impl Credentials {
    pub fn check_api_token(&self) -> Result<()> {
        // TODO: Check API token is valid using the API
        Ok(())
    }
}

impl<'a, S: State> ConnectBuilder<S> {
    pub fn credentials(self, creds: &Credentials) -> ConnectBuilder<SetApiToken<SetBaseurl<S>>>
    where
        S::ApiToken: IsUnset,
        S::Baseurl: IsUnset,
    {
        let api_token = creds.api_token.clone();
        let url = creds.url.clone();
        self.baseurl(url.clone().as_str()).api_token(api_token)
    }
}
