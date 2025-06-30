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

    let context = ComponentsContext::new(&config)
        .map_err(|e| anyhow::anyhow!("Something went wrong when trying to load the Wasm file. Please re-build and try again. {e}"))?;

    match http(context, port, config).await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error starting HTTP server: {e}");
            return Err(e);
        }
    }

    Ok(())
}

pub async fn http(
    component_context: ComponentsContext,
    port: u16,
    config: ComponentsConfiguration,
) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    println!("Listening on http://{addr}");

    loop {
        let (stream, _) = listener.accept().await?;
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
