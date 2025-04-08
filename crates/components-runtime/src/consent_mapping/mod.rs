pub mod versions;

wasmtime::component::bindgen!({
    world: "consent-mapping",
    path: "src/consent_mapping/wit",
    async: true,
});
