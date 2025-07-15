use crate::components::manifest::Manifest;

use edgee_components_runtime::{
    config::{ComponentsConfiguration, EdgeFunctionComponents},
    edge_function::versions::EdgeFunctionWitVersion,
};

use edgee_components_runtime::context::ComponentsContext;
use http::Response;
use http_body_util::{BodyExt, Full};
use std::net::SocketAddr;

use hyper::service::service_fn;
use hyper::{body::Bytes, server::conn::http1};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use std::collections::HashMap;

use notify;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn start_file_watcher(manifest: &Manifest, modified_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
    let exts = match manifest
        .component
        .language
        .clone()
        .unwrap_or("".to_string())
        .as_str()
    {
        "Rust" => vec!["rs", "html"],
        "Python" => vec!["py", "html"],
        "Javascript" => vec!["js", "mjs", "html"],
        "Typescript" => vec!["ts", "html"],
        "Go" => vec!["go", "html"],
        "C" => vec!["c", "h", "html"],
        "C#" => vec!["cs", "html"],
        _ => anyhow::bail!(
            "Unsupported language {} for edge-function component",
            manifest
                .component
                .language
                .clone()
                .unwrap_or("unknown".to_string())
        ),
    };
    let modified_flag_clone = modified_flag.clone();

    tokio::spawn(async move {
        use notify::{RecursiveMode, Watcher};
        use std::{path::Path, sync::mpsc};
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();
        watcher
            .watch(Path::new("."), RecursiveMode::Recursive)
            .unwrap();

        for res in rx {
            match res {
                Ok(event) => {
                    if let notify::EventKind::Modify(_) = event.kind {
                        for path in event.paths {
                            if path
                                .extension()
                                .is_some_and(|ext| exts.contains(&ext.to_str().unwrap()))
                            {
                                modified_flag_clone
                                    .store(true, std::sync::atomic::Ordering::SeqCst);
                                break;
                            }
                        }
                    }
                }
                Err(e) => println!("watch error: {e}"),
            }
        }
    });
    Ok(())
}

pub async fn test_edge_function_component(
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

    let config = ComponentsConfiguration {
        edge_function: vec![EdgeFunctionComponents {
            id: "component".to_string(),
            file: component_path,
            wit_version: EdgeFunctionWitVersion::V1_0_0,
            settings: settings_map,
            ..Default::default()
        }],
        ..Default::default()
    };

    let modified_flag = Arc::new(AtomicBool::new(false));
    if opts.watch {
        start_file_watcher(manifest, modified_flag.clone())?;
    }

    // Start the HTTP server
    match http(opts.port, config, modified_flag, manifest).await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error starting HTTP server: {e}");
            return Err(e);
        }
    }

    Ok(())
}

pub async fn http(
    port: u16,
    config: ComponentsConfiguration,
    modified_flag: Arc<AtomicBool>,
    manifest: &Manifest,
) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    let mut component_context = ComponentsContext::new(&config)
        .map_err(|e| anyhow::anyhow!("Something went wrong when trying to load the Wasm file. Please re-build and try again. {e}"))?;

    println!("Listening on http://{addr}");

    loop {
        let (stream, _) = listener.accept().await?;

        if modified_flag.load(Ordering::SeqCst) {
            println!("Detected source change, rebuilding component...");
            crate::commands::components::build::do_build(manifest, std::path::Path::new("."))
                .await?;
            modified_flag.store(false, std::sync::atomic::Ordering::SeqCst);
            println!("Reloading Wasm component...");
            component_context = ComponentsContext::new(&config)
                .map_err(|e| anyhow::anyhow!("Something went wrong when trying to load the Wasm file. Please re-build and try again. {e}"))?;
        }

        let io = TokioIo::new(stream);

        // Clone the context for each iteration
        let context = component_context.clone();
        let config = config.clone();

        tokio::task::spawn(async move {
            // Create a new service for each connection
            let service = service_fn(move |req| {
                println!("Received request: {req:?}");
                let context = context.clone();
                let config = config.clone();
                async move {
                    let output = edgee_components_runtime::edge_function::invoke_fn(
                        &context,
                        "component",
                        &config,
                        req,
                    )
                    .await;
                    let mut response = Response::builder().status(output.status);

                    for (name, value) in output.headers.iter() {
                        response.headers_mut().unwrap().insert(name, value.clone());
                    }
                    let resp: Response<
                        http_body_util::combinators::BoxBody<Bytes, std::convert::Infallible>,
                    > = response
                        .body(Full::from(Bytes::from(output.body)).boxed())
                        .unwrap();
                    Ok::<_, std::convert::Infallible>(resp)
                }
            });

            // Finally, we bind the incoming connection to our `hello` service
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("Error serving connection: {err}");
            }
        });
    }
}
