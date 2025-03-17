use std::collections::HashMap;

use anyhow::Result;

use event_builder::{IsComplete, State};

const TELEMETRY_BASE_URL: &str = "https://edgee-cli.edgee.team";

pub fn setup() -> Result<()> {
    Ok(())
}

#[derive(Debug, bon::Builder)]
#[builder(on(String, into))]
pub struct Event {
    name: String,
    title: String,
    #[builder(default)]
    properties: HashMap<String, String>,
}

impl Event {
    pub async fn send(self) -> Result<()> {
        use edgee_api_client::data_collection as dc;

        let client = dc::new()
            .baseurl(TELEMETRY_BASE_URL)
            .debug_mode(true)
            .connect();

        let track = dc::types::EdgeeEventTrack::builder().data(
            dc::types::EdgeeEventTrackData::builder()
                .name(self.name)
                .properties(self.properties),
        );
        let events = vec![dc::types::EdgeeEventDataCollectionEventsItem::track(track)?];

        let page = dc::types::EdgeeEventPageData::builder()
            .title(self.title)
            .url("cli://edgee-cli")
            .path("/");
        let context =
            dc::types::EdgeeEventDataCollectionContext::builder().page(Some(page.try_into()?));

        let payload = dc::types::EdgeeEvent::builder().data_collection(
            dc::types::EdgeeEventDataCollection::builder()
                .events(events)
                .context(Some(context.try_into()?)),
        );

        let res = client
            .collect_event()
            .body(payload)
            .send()
            .await
            .inspect_err(|err| {
                dbg!(err);
            })?;
        dbg!(res.headers());
        dbg!(res);

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
