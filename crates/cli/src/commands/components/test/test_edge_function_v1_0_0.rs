use crate::components::manifest::Manifest;
use http::HeaderValue;
use wasmtime_wasi_http::WasiHttpView;

use edgee_components_runtime::{
    config::{ComponentsConfiguration, EdgeFunctionComponents},
    edge_function::versions::EdgeFunctionWitVersion,
};

use edgee_components_runtime::context::ComponentsContext;
use http_body_util::BodyExt;

use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use tokio::net::TcpListener;

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

    let config = ComponentsConfiguration {
        edge_function: vec![EdgeFunctionComponents {
            id: "component".to_string(),
            file: component_path.to_string(),
            wit_version: EdgeFunctionWitVersion::V1_0_0,
            ..Default::default()
        }],
        ..Default::default()
    };

    let port = opts.port;

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

    let context = ComponentsContext::new(&config)
        .map_err(|e| anyhow::anyhow!("Something went wrong when trying to load the Wasm file. Please re-build and try again. {e}"))?;

    println!("Component loaded successfully: {}", component_path);
    match http(context, port, settings_map).await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error starting HTTP server: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

fn build_response(
    status: http::StatusCode,
    body: String,
) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::builder()
        .status(status)
        .body(Full::new(Bytes::from(body)))
        .unwrap())
}
async fn component_call(
    component_context: ComponentsContext,
    settings: HashMap<String, String>,
    mut req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut store = component_context.empty_store_with_stdout();
    let data = store.data_mut();

    let settings = serde_json::to_string(&settings).unwrap_or_default();
    let Ok(settings_header) = HeaderValue::from_str(&settings) else {
        return build_response(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to serialize settings".to_string(),
        );
    };
    req.headers_mut()
        .insert("x-edgee-component-settings", settings_header);

    println!("Received request: {:?}", req);
    let wasi_req = data
        .new_incoming_request(wasmtime_wasi_http::bindings::http::types::Scheme::Http, req)
        .unwrap();
    let out = data.new_response_outparam(sender).unwrap();

    let component = component_context
        .get_edge_function_1_0_0_instance("component", &mut store)
        .await
        .unwrap();

    // call the WASI HTTP handler
    tokio::task::spawn(async move {
        match component
            .wasi_http_incoming_handler()
            .call_handle(store, wasi_req, out)
            .await
        {
            Ok(()) => {}
            Err(e) => {
                println!("WASI HTTP handler failed: {:?}", e);
            }
        }
    });

    // wait for data to stream from the WASI HTTP handler
    match receiver.await {
        // If the client calls `response-outparam::set` then one of these
        // methods will be called.
        Ok(Ok(response)) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = match response.into_body().collect().await {
                Ok(body) => body.to_bytes().to_vec(),
                Err(e) => {
                    println!("Failed to collect response body: {:?}", e);
                    return build_response(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to collect response body".to_string(),
                    );
                }
            };

            let mut builder = Response::builder().status(status);
            let builder_headers = builder.headers_mut().unwrap();
            for (header_name, header_value) in headers.iter() {
                builder_headers.insert(header_name, header_value.clone());
            }
            // return the response with the body
            Ok(builder.body(body.into()).unwrap())
        }

        Ok(Err(_)) => build_response(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to handle response".to_string(),
        )
,
        // Otherwise the `sender` will get dropped along with the `Store`
        // meaning that the oneshot will get disconnected and here we can
        // inspect the `task` result to see what happened
        Err(_) => build_response(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to receive response from WASI HTTP handler".to_string()
        )
    }
}

pub async fn http(
    component_context: ComponentsContext,
    port: u16,
    settings: HashMap<String, String>,
) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    println!("Listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        // Clone the context for each iteration
        let context = component_context.clone();
        let settings = settings.clone();

        tokio::task::spawn(async move {
            // Create a new service for each connection
            let service = service_fn(move |req| {
                let context = context.clone();
                let settings = settings.clone();
                async move { component_call(context, settings, req).await }
            });

            // Finally, we bind the incoming connection to our `hello` service
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
