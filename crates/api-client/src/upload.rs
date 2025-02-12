use std::path::Path;

use anyhow::Result;

use super::Client;

impl Client {
    pub async fn upload_file(&self, path: &Path) -> Result<String> {
        let presigned_url = self.get_upload_presigned_url().send().await?;
        let upload_url = &presigned_url.upload_url;

        let content = std::fs::read(path)?;

        let client = reqwest::Client::new();
        let res = client.put(upload_url).body(content).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("Could not upload file");
        }

        Ok(upload_url.clone())
    }
}
