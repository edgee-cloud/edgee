pub mod components;
pub mod payload;

wasmtime::component::bindgen!({
    world: "data-collection",
    path: "wit/",
    async: true,
});
