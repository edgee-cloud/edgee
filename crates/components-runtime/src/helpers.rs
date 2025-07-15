use crate::config::ComponentSource;
use wasmtime::component::Component;
use wasmtime::Engine;

pub(crate) fn instanciate_component(
    engine: &Engine,
    component_source: &ComponentSource,
) -> anyhow::Result<Component> {
    // Attempt to deserialize from a serialized file if available
    if let Some(serialized_file) = &component_source.serialized_file {
        tracing::debug!("Deserializing component from serialized file: {serialized_file}");
        if let Ok(bytes) = std::fs::read(serialized_file) {
            match unsafe {
                // Ensure the serialized file is trusted before using this unsafe block

                Component::deserialize(engine, bytes)
            } {
                Ok(component) => return Ok(component),
                Err(e) => {
                    tracing::debug!(
                        "Failed to deserialize component from file: {serialized_file}, error: {e}"
                    );
                    // Continue to the next method of instantiation
                }
            }
        }
    }

    tracing::debug!("Creating component from file: {}", component_source.file);
    // Fall back to loading the component from a file
    Component::from_file(engine, &component_source.file)
}
