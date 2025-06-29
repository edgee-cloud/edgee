pub mod versions;

use std::convert::Infallible;
use wasmtime::Store;
use wasmtime_wasi_http::types::HostIncomingRequest;
use wasmtime_wasi_http::WasiHttpView;

use crate::config::ComponentsConfiguration;

use crate::context::ComponentsContext;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use hyper::body::Incoming;

use http_body_util::Full;
use hyper::Response;

fn build_response(
    status: http::StatusCode,
    body: String,
) -> Result<Response<BoxBody<Bytes, Infallible>>, anyhow::Error> {
    Ok(Response::builder()
        .status(status)
        .body(Full::from(Bytes::from(body)).boxed())
        .unwrap())
}

async fn invoke_fn_internal(
    component_ctx: &ComponentsContext,
    component_name: &str,
    request: wasmtime::component::Resource<HostIncomingRequest>,
    mut store: Store<crate::context::HostState>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, anyhow::Error> {
    let data = store.data_mut();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let response = data.new_response_outparam(sender).unwrap();
    let component = component_ctx
        .get_edge_function_1_0_0_instance(component_name, &mut store)
        .await
        .unwrap();

    // call the WASI HTTP handler
    let task = tokio::task::spawn(async move {
        match component
            .wasi_http_incoming_handler()
            .call_handle(store, request, response)
            .await
        {
            Ok(()) => {}
            Err(e) => {
                println!("WASI HTTP handler failed: {:?}", e);
            }
        }
    });

    // wait for data to stream from the WASI HTTP handler
    let resp = match receiver.await {
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
            Ok(builder.body(Full::from(body).boxed()).unwrap())
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
        };
    resp
}

pub async fn invoke_fn(
    component_ctx: &ComponentsContext,
    component_name: &str,
    component_configs: &ComponentsConfiguration,
    mut request: http::Request<Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>, anyhow::Error> {
    let mut store = component_ctx.empty_store_with_stdout();
    let data = store.data_mut();

    if component_configs.edge_function.is_empty() {
        return build_response(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "No edge function configured".to_string(),
        );
    }

    // grab the component configuration for the given component name
    let Some(component_config) = component_configs.edge_function.iter().find_map(|f| {
        if f.id == component_name {
            Some(f.clone())
        } else {
            None
        }
    }) else {
        return build_response(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Component configuration not found for: {}", component_name),
        );
    };

    // Set the component settings as a header
    let settings = serde_json::to_string(&component_config.settings).unwrap_or_default();
    let Ok(settings_header) = http::HeaderValue::from_str(&settings) else {
        return build_response(
            http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Component configuration not found for: {}", component_name),
        );
    };
    request
        .headers_mut()
        .insert("x-edgee-component-settings", settings_header);

    let wasi_req = data
        .new_incoming_request(
            wasmtime_wasi_http::bindings::http::types::Scheme::Http,
            request,
        )
        .unwrap();
    // Invoke the WASI HTTP handler
    invoke_fn_internal(component_ctx, component_name, wasi_req, store).await
}
