use chrono::Utc;
use colored::Colorize;
use colored_json::prelude::*;
use edgee_components_runtime::config::{ComponentsConfiguration, DataCollectionComponents};
use edgee_components_runtime::context::ComponentsContext;
use std::collections::HashMap;

use edgee_components_runtime::data_collection;
use edgee_components_runtime::data_collection::payload::{Event, EventType};
use http::{HeaderMap, HeaderName, HeaderValue};
use std::str::FromStr;

use edgee_components_runtime::data_collection::versions::v1_0_0::data_collection::exports::edgee::components::data_collection::EdgeeRequest;
use edgee_components_runtime::data_collection::versions::v1_0_0::data_collection::exports::edgee::components::data_collection::HttpMethod;
use crate::components::manifest::Manifest;

trait IntoCurl {
    fn to_curl(&self) -> String;
}

impl IntoCurl for HttpMethod {
    fn to_curl(&self) -> String {
        match self {
            HttpMethod::Get => "GET".to_string(),
            HttpMethod::Post => "POST".to_string(),
            HttpMethod::Put => "PUT".to_string(),
            HttpMethod::Delete => "DELETE".to_string(),
        }
    }
}

impl IntoCurl for EdgeeRequest {
    fn to_curl(&self) -> String {
        let mut curl = format!("curl -X {} {}", self.method.to_curl(), self.url);
        if !self.body.is_empty() {
            curl.push_str(&format!(" -d '{}'", self.body));
        }
        curl
    }
}

pub async fn test_data_collection_component_1_0_0(
    opts: super::Options,
    manifest: &Manifest,
) -> anyhow::Result<()> {
    let component_path = manifest
        .component
        .build
        .output_path
        .clone()
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow::anyhow!("Invalid path"))?;

    if !std::path::Path::new(&component_path).exists() {
        return Err(anyhow::anyhow!("Output path not found in manifest file.",));
    }

    let config = ComponentsConfiguration {
        data_collection: vec![DataCollectionComponents {
            id: component_path.to_string(),
            file: component_path.to_string(),
            wit_version: match manifest.component.wit_version.as_str() {
                "1.0.0" => {
                    edgee_components_runtime::data_collection::versions::DataCollectionWitVersion::V1_0_0
                },
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported wit version: {}",
                        manifest.component.wit_version
                    ));
                }
            },
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = ComponentsContext::new(&config)
        .map_err(|e| anyhow::anyhow!("Something went wrong when trying to load the Wasm file. Please re-build and try again. {e}"))?;

    let mut store = context.empty_store_with_stdout();

    let instance = context
        .get_data_collection_1_0_0_instance(&component_path, &mut store)
        .await?;
    let component = instance.edgee_components_data_collection();

    // events generated with demo.edgee.app
    let page_event_json = r#"[{"uuid":"37009b9b-a572-4615-87c1-09e257331ecb","timestamp":"2025-02-03T15:46:39.283317613Z","type":"page","data":{"keywords":["demo","tag manager"],"title":"Page with Edgee components","url":"https://demo.edgee.app/analytics-with-edgee.html","path":"/analytics-with-edgee.html","referrer":"https://demo.edgee.dev/analytics-with-js.html"},"context":{"page":{"keywords":["demo","tag manager"],"title":"Page with Edgee components","url":"https://demo.edgee.app/analytics-with-edgee.html","path":"/analytics-with-edgee.html","referrer":"https://demo.edgee.dev/analytics-with-js.html"},"user":{"edgee_id":"6bb171d5-2284-41ee-9889-91af03b71dc5"},"client":{"ip":"127.0.0.1","locale":"en-us","accept_language":"en-US,en;q=0.9","timezone":"Europe/Paris","user_agent":"Mozilla/5.0 (X11; Linux x86_64)AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36","user_agent_version_list":"Not A(Brand;8|Chromium;132","user_agent_mobile":"0","os_name":"Linux","user_agent_architecture":"x86","user_agent_bitness":"64","user_agent_full_version_list":"Not A(Brand;8.0.0.0|Chromium;132.0.6834.159","user_agent_model":"","os_version":"6.12.11","screen_width":1920,"screen_height":1280,"screen_density":1.5},"session":{"session_id":"1738597536","session_count":1,"session_start":false,"first_seen":"2025-02-03T15:45:36.569004889Z","last_seen":"2025-02-03T15:46:39.278740029Z"}},"from":"edge"}]"#;
    let track_event_json = r#" [{"uuid":"4cffe10b-b5fd-429e-96d2-471f0575005f","timestamp":"2025-02-03T16:06:32.809486270Z","type":"track","data":{"name":"button_click","properties":{"registered":false,"size":10,"color":"blue","category":"shoes","label":"Blue Sneakers"}},"context":{"page":{"keywords":["demo","tag manager"],"title":"Page with Edgee components","url":"https://demo.edgee.app/analytics-with-edgee.html","path":"/analytics-with-edgee.html","referrer":"https://demo.edgee.dev/"},"user":{"user_id":"123456","anonymous_id":"anon-123","edgee_id":"69659401-40cf-4ac8-8ffc-630a10fe06dc","properties":{"verified":true,"age":42,"email":"me@example.com","name":"John Doe"}},"client":{"ip":"127.0.0.1","locale":"en-us","accept_language":"en-US,en;q=0.5","timezone":"Europe/Paris","user_agent":"Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0","screen_width":1440,"screen_height":960,"screen_density":2.0},"session":{"session_id":"1738598699","session_count":7,"session_start":false,"first_seen":"2024-12-12T16:30:03.693248190Z","last_seen":"2025-02-03T16:06:32.808844970Z"}},"from":"client","consent":"granted"}]"#;
    let user_event_json = r#"[{"uuid":"eb0f001a-cd2b-42c4-9c71-7b1c2bcda445","timestamp":"2025-02-03T16:07:04.878715197Z","type":"user","data":{"user_id":"123456","anonymous_id":"anon-123","edgee_id":"69659401-40cf-4ac8-8ffc-630a10fe06dc","properties":{"age":42,"verified":true,"name":"John Doe","email":"me@example.com"}},"context":{"page":{"keywords":["demo","tag manager"],"title":"Page with Edgee components","url":"https://demo.edgee.app/analytics-with-edgee.html","path":"/analytics-with-edgee.html","referrer":"https://demo.edgee.dev/"},"user":{"user_id":"123456","anonymous_id":"anon-123","edgee_id":"69659401-40cf-4ac8-8ffc-630a10fe06dc","properties":{"email":"me@example.com","age":42,"name":"John Doe","verified":true}},"client":{"ip":"127.0.0.1","locale":"en-us","accept_language":"en-US,en;q=0.5","timezone":"Europe/Paris","user_agent":"Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0","screen_width":1440,"screen_height":960,"screen_density":2.0},"session":{"session_id":"1738598699","session_count":7,"session_start":false,"first_seen":"2024-12-12T16:30:03.693248190Z","last_seen":"2025-02-03T16:07:04.878137016Z"}},"from":"client","consent":"granted"}]"#;
    //
    let mut page_event = serde_json::from_str::<Vec<Event>>(page_event_json).unwrap()[0].clone();
    let mut track_event = serde_json::from_str::<Vec<Event>>(track_event_json).unwrap()[0].clone();
    let mut user_event = serde_json::from_str::<Vec<Event>>(user_event_json).unwrap()[0].clone();

    // set the timestamp to now
    page_event.timestamp = Utc::now();
    track_event.timestamp = Utc::now();
    user_event.timestamp = Utc::now();

    // read the event file if provided

    let custom_event_data = match opts.events_file {
        Some(file) => {
            if !std::path::Path::new(&file).exists() {
                return Err(anyhow::anyhow!("Event file not found: {}", file));
            }
            let data = std::fs::read_to_string(file);
            data.ok()
        }
        None => {
            // use default events if no file is provided
            None
        }
    };

    // setting management
    let mut settings_map = HashMap::new();

    // insert user provided settings
    match (opts.settings, opts.settings_file) {
        (Some(_), Some(_)) => {
            return Err(anyhow::anyhow!(
                "Please provide either settings or settings-file, not both"
            ));
        }
        (None, None) => {}
        (Some(settings), None) => {
            for (key, value) in settings {
                settings_map.insert(key, value);
            }
        }
        (None, Some(settings_file)) => {
            #[derive(serde::Deserialize)]
            struct Settings {
                settings: HashMap<String, String>,
            }

            let settings_file = std::fs::read_to_string(settings_file)?;
            let config: Settings = toml::from_str(&settings_file).expect("Failed to parse TOML");

            for (key, value) in config.settings {
                settings_map.insert(key, value);
            }
        }
    }

    // check that all required settings are provided
    for (name, setting) in &manifest.component.settings {
        if setting.required && !settings_map.contains_key(name) {
            return Err(anyhow::anyhow!("missing required setting {}", name));
        }
    }

    for name in settings_map.keys() {
        if !manifest.component.settings.contains_key(name) {
            return Err(anyhow::anyhow!("unknown setting {}", name));
        }
    }

    let settings = settings_map.clone().into_iter().collect();
    // select events to run
    let mut events = vec![];
    match opts.event_type {
        None => {
            match custom_event_data {
                Some(data) => {
                    // parse the custom event data
                    let custom_events: Vec<Event> = serde_json::from_str(&data)
                        .map_err(|e| anyhow::anyhow!("Failed to parse custom event data: {}", e))?;
                    events.extend(custom_events);
                }
                None => {
                    // use default events if no custom data is provided
                    events.push(page_event);
                    events.push(track_event);
                    events.push(user_event);
                }
            }
        }
        Some(event_type) => match event_type.as_str() {
            "page" => {
                events.push(page_event);
            }
            "track" => {
                events.push(track_event);
            }
            "user" => {
                events.push(user_event);
            }
            _ => {
                return Err(anyhow::anyhow!("Invalid event type"));
            }
        },
    }

    if opts.display_input {
        println!(
            "{}: {}",
            "Settings".green(),
            serde_json::to_string_pretty(&settings_map)?.to_colored_json_auto()?
        );
    }
    for event in events {
        println!("---------------------------------------------------");
        let request = match event.event_type {
            EventType::Page => {
                tracing::info!("Running test with {} event\n", "page".green(),);
                component
                    .call_page(&mut store, &event.clone().into(), &settings)
                    .await
            }
            EventType::Track => {
                tracing::info!("Running test with {} event\n", "track".green(),);
                component
                    .call_track(&mut store, &event.clone().into(), &settings)
                    .await
            }
            EventType::User => {
                tracing::info!("Running test with {} event\n", "user".green(),);
                component
                    .call_user(&mut store, &event.clone().into(), &settings)
                    .await
            }
        };

        let request = match request {
            Ok(Ok(request)) => request,
            Err(e) => return Err(anyhow::anyhow!("Failed to call component: {e}")),
            Ok(Err(e)) => return Err(anyhow::anyhow!("Failed to call component: {e}")),
        };

        if opts.display_input {
            tracing::info!("Input event:\n");
            println!(
                "{}: {}\n",
                "Event".green(),
                serde_json::to_string_pretty(&event)?.to_colored_json_auto()?
            );
        }

        let mut headers = HeaderMap::new();
        for (key, value) in request.headers.iter() {
            headers.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
        }

        if request.forward_client_headers {
            let _ = data_collection::insert_expected_headers(&mut headers, &event, &HashMap::new());
        }

        tracing::info!("Output from Wasm:");
        println!("\n{} {{", "EdgeeRequest".green());
        println!("\t{}: {:#?}", "Method".green(), request.method);
        println!("\t{}: {}", "URL".green(), request.url.green());

        let pretty_headers: HashMap<String, String> = headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
            .collect();
        println!(
            "\t{}: {}",
            "Headers".green(),
            serde_json::to_string_pretty(&pretty_headers)?
                .to_colored_json_auto()?
                .replace("\n", "\n\t")
        );
        println!(
            "\t{}: {}",
            "Forward Client Headers".green(),
            request.forward_client_headers
        );
        if let Ok(pretty_json) = serde_json::from_str::<serde_json::Value>(&request.body) {
            println!(
                "\t{}: {}",
                "Body".green(),
                serde_json::to_string_pretty(&pretty_json)?
                    .to_colored_json_auto()?
                    .replace("\n", "\n\t")
            );
        } else {
            println!("\t{}: {:#?}", "Body".green(), request.body);
        }
        println!("}}");

        if opts.curl {
            print!("{} ", &request.to_curl());
            for (key, value) in headers.iter() {
                print!("-H \"{}\":\"{}\" ", key, value.to_str()?);
            }
            println!();
        }

        if opts.make_http_request {
            run_request(request.clone()).await?;
        }
    }

    Ok(())
}

async fn run_request(request: EdgeeRequest) -> anyhow::Result<()> {
    use std::str::FromStr;
    use std::time::Duration;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let mut headers = HeaderMap::new();
    for (key, value) in request.headers.iter() {
        headers.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
    }

    let res = match request.method {
        HttpMethod::Get => client.get(request.url).headers(headers).send().await,
        HttpMethod::Put => {
            client
                .put(request.url)
                .headers(headers)
                .body(request.body)
                .send()
                .await
        }
        HttpMethod::Post => {
            client
                .post(request.url)
                .headers(headers)
                .body(request.body)
                .send()
                .await
        }
        HttpMethod::Delete => client.delete(request.url).headers(headers).send().await,
    };

    tracing::info!("HTTP response:");
    match res {
        Ok(res) => {
            println!("\n{}: {}", "Response".green(), res.status());
            let headers = res.headers();
            let mut pretty_headers = HashMap::new();
            for (key, value) in headers {
                pretty_headers.insert(key.to_string(), value.to_str()?.to_string());
            }
            println!(
                "{}: {}",
                "Headers".green(),
                serde_json::to_string_pretty(&pretty_headers)?
                    .to_colored_json_auto()?
                    .replace("\n", "\n\t")
            );
            let body = res.text().await?;
            let pretty_json = serde_json::from_str::<serde_json::Value>(&body);
            match pretty_json {
                Ok(pretty_json) => {
                    println!(
                        "{}: {}",
                        "Body".green(),
                        serde_json::to_string_pretty(&pretty_json)?
                            .to_colored_json_auto()?
                            .replace("\n", "\n\t")
                    );
                }
                Err(_) => {
                    println!("{}: {:#?}", "Body".green(), body);
                }
            }
        }
        Err(e) => {
            println!("{}: {}", "Error".red(), e);
        }
    }

    Ok(())
}
