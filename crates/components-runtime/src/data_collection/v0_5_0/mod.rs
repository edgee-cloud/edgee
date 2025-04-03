mod convert;

pub mod data_collection {
    wasmtime::component::bindgen!({
        world: "data-collection-zero-five-zero",
        path: "wit/",
        async: true,
    });
}
