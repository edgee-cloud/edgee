use std::future::Future;
use std::{collections::HashMap, time::Duration};

use anyhow::Result;

use event_builder::{IsComplete, IsUnset, SetProperties, State};

const TELEMETRY_BASE_URL: &str = "https://edgee-cli.edgee.team";
const TELEMETRY_TIMEOUT: Duration = Duration::from_millis(1000);
const TELEMETRY_WARNING: &str = r#"Welcome to the Edgee CLI!

Telemetry
---------
The Edgee CLI collect usage data in order to help us improve your experience.
You can opt-out of telemetry by setting the EDGEE_TELEMETRY_OPTOUT environment
variable to '1' or 'true' using your favorite shell.
"#;

pub fn setup() -> Result<()> {
    let state_dir = dirs::state_dir()
        .or_else(dirs::config_dir)
        .ok_or_else(|| anyhow::anyhow!("state dir is not existent"))?
        .join("edgee");
    std::fs::create_dir_all(&state_dir)?;

    let first_run = state_dir.join("first_run.marker");
    if !first_run.exists() {
        eprintln!("{TELEMETRY_WARNING}");

        drop(std::fs::File::create_new(&first_run)?);
    }

    Ok(())
}

pub async fn process_cli_command<F, T, E>(f: F) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    let args = std::env::args()
        .skip(1)
        .take_while(|arg| !arg.starts_with("-"))
        .collect::<Vec<_>>()
        .join(" ");

    let res = f.await;

    let _ = Event::builder()
        .name("command")
        .title(&args)
        .with_properties(|properties| {
            let os_info = os_info::get();

            properties.insert(
                "os".to_string(),
                format!(
                    "{} ({}, {})",
                    os_info.os_type(),
                    os_info.bitness(),
                    os_info.version(),
                ),
            );
            properties.insert(
                "edgee_version".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            );

            properties.insert("command".to_string(), args.clone());
            properties.insert(
                "result".to_string(),
                if res.is_ok() {
                    "ok".to_string()
                } else {
                    "error".to_string()
                },
            );
        })
        .send()
        .await;

    res
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

        if let Ok(value) = std::env::var("EDGEE_TELEMETRY_OPTOUT") {
            if value == "1" || value == "true" {
                return Ok(());
            }
        }

        let client = dc::new()
            .baseurl(TELEMETRY_BASE_URL)
            .debug_mode(true)
            .with_client_builder(|builder| builder.timeout(TELEMETRY_TIMEOUT))
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

        let _res = client
            .collect_event()
            .body(payload)
            .send()
            .await
            .inspect_err(|err| {
                dbg!(err);
            })?;

        Ok(())
    }
}

impl<S: State> EventBuilder<S> {
    pub fn with_properties(
        self,
        f: impl Fn(&mut HashMap<String, String>),
    ) -> EventBuilder<SetProperties<S>>
    where
        S::Properties: IsUnset,
    {
        let mut properties = Default::default();
        f(&mut properties);
        self.properties(properties)
    }

    pub async fn send(self) -> Result<()>
    where
        S: IsComplete,
    {
        self.build().send().await
    }
}
