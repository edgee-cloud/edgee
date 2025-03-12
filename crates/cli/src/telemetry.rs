use std::collections::HashMap;

use anyhow::Result;

use event_builder::{IsComplete, State};

const TELEMETRY_BASE_URL: &str = "https://edgee-cli.edgee.team";

#[derive(Debug, bon::Builder)]
#[builder(on(String, into))]
pub struct Event {
    name: String,
    #[builder(default)]
    properties: HashMap<String, String>,
}

impl Event {
    pub async fn send(self) -> Result<()> {
        use reqwest::header;
        use reqwest::Client;

        let client = Client::builder()
            .user_agent(format!("edgee/{}", env!("CARGO_PKG_VERSION")))
            .build()?;

        let payload = serde_json::json!({
            "data_collection": {
                "events": [
                    {
                        "type": "user",
                        "data": {
                            "name": self.name,
                            "properties": self.properties,
                        }
                    }
                ],
                "context": {
                    "user": {
                        // "user_id": "3dc6a439-1d61-4054-a9b9-0634260ff866",
                    },
                },
            }
        });

        let req = client
            .post(format!("{}/_edgee/event", TELEMETRY_BASE_URL))
            .header(header::COOKIE, "_edgeedebug=true")
            .json(&payload)
            .build()?;
        dbg!(&req);
        let res = client.execute(req).await?;
        let body: serde_json::Value = res.json().await?;
        dbg!(&body);

        Ok(())
    }
}

impl<S: State> EventBuilder<S> {
    pub async fn send(self) -> Result<()>
    where
        S: IsComplete,
    {
        self.build().send().await
    }
}
