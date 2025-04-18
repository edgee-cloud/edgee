use crate::config::DataCollectionComponents;
use crate::context::HostState;
use crate::data_collection::insert_expected_headers;
use crate::data_collection::payload::EventType;
use crate::data_collection::versions::v1_0_1::data_collection::exports::edgee::components1_0_1::data_collection as DC;
use crate::{context::ComponentsContext, data_collection::payload};
use http::{HeaderMap, HeaderName, HeaderValue};
use std::str::FromStr;
use tracing::error;
use wasmtime::Store;

pub async fn get_edgee_request(
    event: &payload::Event,
    component_ctx: &ComponentsContext,
    cfg: &DataCollectionComponents,
    store: &mut Store<HostState>,
) -> Result<(HeaderMap, String, String, String), anyhow::Error> {
    let instance = match component_ctx
        .get_data_collection_1_0_1_instance(&cfg.id, store)
        .await
    {
        Ok(instance) => instance,
        Err(err) => {
            error!("Failed to get data collection instance. Error: {}", err);
            return Err(err);
        }
    };
    let component = instance.edgee_components1_0_1_data_collection();

    let component_settings: Vec<(String, String)> = cfg
        .settings
        .additional_settings
        .clone()
        .into_iter()
        .collect();

    // call the corresponding method of the component
    let request = match event.event_type {
        EventType::Page => {
            component
                .call_page(store, &event.clone().into(), &component_settings)
                .await
        }
        EventType::Track => {
            component
                .call_track(store, &event.clone().into(), &component_settings)
                .await
        }
        EventType::User => {
            component
                .call_user(store, &event.clone().into(), &component_settings)
                .await
        }
    };
    let request = match request {
        Ok(Ok(request)) => request,
        Ok(Err(err)) => {
            // todo: debug and trace response (error)
            error!(
                step = "request",
                err = err.to_string(),
                "failed to handle data collection payload"
            );
            return Err(anyhow::anyhow!(err));
        }
        Err(err) => {
            // todo: debug and trace response (error)
            error!(
                step = "request",
                err = err.to_string(),
                "failed to handle data collection payload"
            );
            return Err(anyhow::anyhow!(err));
        }
    };

    let mut headers = HeaderMap::new();
    for (key, value) in request.headers.iter() {
        headers.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
    }

    if request.forward_client_headers {
        let _ = insert_expected_headers(&mut headers, event);
    }

    let method = match request.method {
        DC::HttpMethod::Get => "GET",
        DC::HttpMethod::Put => "PUT",
        DC::HttpMethod::Post => "POST",
        DC::HttpMethod::Delete => "DELETE",
    }
    .to_string();

    Ok((headers, method, request.url, request.body))
}
