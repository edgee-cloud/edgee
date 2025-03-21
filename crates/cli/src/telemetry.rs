use std::future::Future;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::{collections::HashMap, time::Duration};

use anyhow::Result;

use edgee_api_client::types::UserWithRoles;
use event_builder::{IsComplete, IsUnset, SetProperties, State};
use serde::{Deserialize, Serialize};

static STATE_DIR: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    dirs::state_dir()
        .or_else(dirs::config_dir)
        .map(|path| path.join("edgee"))
});

const TELEMETRY_BASE_URL: &str = "https://edgee-cli.edgee.app";
const TELEMETRY_TIMEOUT: Duration = Duration::from_millis(1000);
const TELEMETRY_WARNING: &str = r#"Welcome to the Edgee CLI!

Telemetry
---------
The Edgee CLI collect usage data in order to help us improve your experience.
You can opt-out of telemetry by setting the EDGEE_TELEMETRY_OPTOUT environment
variable to '1' or 'true' using your favorite shell.
"#;

#[derive(Debug, Serialize, Deserialize)]
struct Data {
    is_logged_in: bool,
    id: String,
}

impl Data {
    const FILENAME: &str = "telemetry.json";

    fn new() -> Self {
        Data {
            is_logged_in: false,
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    fn load() -> Result<Self> {
        let data_file = STATE_DIR
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no state dir"))?
            .join(Self::FILENAME);
        let f = std::fs::File::open(data_file)?;
        serde_json::from_reader(f).map_err(Into::into)
    }
}

pub fn setup() -> Result<()> {
    let state_dir = STATE_DIR
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("state dir is not existent"))?;
    std::fs::create_dir_all(state_dir)?;

    let data_file = state_dir.join(Data::FILENAME);
    if !data_file.exists() {
        // If data file is not here it means it's first run
        println!("{TELEMETRY_WARNING}");

        let f = std::fs::File::create_new(&data_file)?;
        serde_json::to_writer(f, &Data::new())?;
    }

    Ok(())
}

pub fn login(user: &UserWithRoles) -> Result<()> {
    let data = Data {
        is_logged_in: true,
        id: user.id.clone(),
    };

    let data_file = STATE_DIR
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no state dir"))?
        .join(Data::FILENAME);
    let f = std::fs::File::create(data_file)?;

    serde_json::to_writer(f, &data).map_err(Into::into)
}

pub fn is_telemetry_enabled() -> bool {
    let Ok(value) = std::env::var("EDGEE_TELEMETRY_OPTOUT") else {
        return true;
    };
    value != "1" && value != "true"
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

        if !is_telemetry_enabled() {
            return Ok(());
        }

        let data = Data::load()?;

        let client = dc::new()
            .baseurl(
                std::env::var("EDGEE_TELEMETRY_BASEURL")
                    .unwrap_or_else(|_| TELEMETRY_BASE_URL.to_string()),
            )
            .debug_mode(std::env::var("EDGEE_TELEMETRY_DEBUG").is_ok_and(|value| value == "1"))
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
        let user = dc::types::EdgeeEventUserData::builder();
        let user = if data.is_logged_in {
            user.user_id(Some(data.id))
        } else {
            user.anonymous_id(Some(data.id))
        };
        let context = dc::types::EdgeeEventDataCollectionContext::builder()
            .page(Some(page.try_into()?))
            .user(Some(user.try_into()?));

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
                tracing::debug!("Telemetry error: {err}");
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
