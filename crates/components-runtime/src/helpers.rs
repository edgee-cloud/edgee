use crate::config::ComponentSource;
use wasmtime::component::Component;
use wasmtime::Engine;

pub(crate) fn instanciate_component(
    engine: &Engine,
    component_source: &ComponentSource,
) -> anyhow::Result<Component> {
    // Attempt to deserialize from a serialized buffer if available
    if let Some(serialized_buff) = &component_source.serialized_binary {
        tracing::debug!("Deserializing component from serialized buffer");
        match unsafe {
            // Ensure the serialized buffer is trusted before using this unsafe block
            Component::deserialize(engine, serialized_buff)
        } {
            Ok(component) => return Ok(component),
            Err(e) => {
                tracing::debug!("Failed to deserialize component from buffer: {e}");
                // Continue to the next method of instantiation
            }
        }
    }

    println!("No serialized buffer found, proceeding with other methods...");

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

    // Attempt to create a component from a binary buffer if available
    if let Some(binary) = &component_source.binary {
        tracing::debug!("Creating component from binary buffer");
        if let Ok(component) = Component::new(engine, binary) {
            return Ok(component);
        }
    }

    tracing::debug!("Creating component from file: {}", component_source.file);
    // Fall back to loading the component from a file
    Component::from_file(engine, &component_source.file)
}
