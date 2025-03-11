use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    connect_builder::{IsUnset, SetApiToken, SetBaseurl, State},
    ConnectBuilder,
};

#[derive(Debug, Deserialize, Default, Serialize, Clone)]
pub struct Config {
    #[serde(default)]
    api_token: Option<String>,
    #[serde(default)]
    url: Option<String>,

    #[serde(flatten)]
    profiles: std::collections::HashMap<String, Credentials>,
}

#[derive(Debug, Deserialize, Default, Serialize, Clone)]
pub struct Credentials {
    pub api_token: String,
    #[serde(default)]
    pub url: Option<String>,
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
        // if EDGEE_API_URL and EDGEE_API_TOKEN are set, use them
        // skip using the credentials file
        if let Ok(api_token) = std::env::var("EDGEE_API_TOKEN") {
            return Ok(Self {
                api_token: Some(api_token),
                url: Some(
                    std::env::var("EDGEE_API_URL").unwrap_or("https://api.edgee.app".to_string()),
                ),
                ..Default::default()
            });
        };

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
            Some(profile) => self.profiles.get(profile).cloned(),
            None => match (self.api_token.clone(), self.url.clone()) {
                (Some(api_token), Some(url)) => Some(Credentials {
                    api_token,
                    url: Some(url),
                }),
                (Some(api_token), _) => Some(Credentials {
                    api_token,
                    url: Some("https://api.edgee.app".to_string()),
                }),
                _ => None,
            },
        }
    }

    pub fn set(&mut self, profile: Option<String>, creds: Credentials) {
        match profile {
            Some(profile) => {
                self.profiles.insert(profile, creds);
            }
            None => {
                self.api_token = Some(creds.api_token);
                self.url = creds.url;
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

impl<S: State> ConnectBuilder<S> {
    pub fn credentials(self, creds: &Credentials) -> ConnectBuilder<SetApiToken<SetBaseurl<S>>>
    where
        S::ApiToken: IsUnset,
        S::Baseurl: IsUnset,
    {
        let api_token = creds.api_token.clone();
        let url = creds.url.clone();
        self.baseurl(url.unwrap_or("https://api.edgee.app".to_string()))
            .api_token(api_token)
    }
}
