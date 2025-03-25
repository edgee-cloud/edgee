mod convert;

pub mod data_collection {
    wasmtime::component::bindgen!({
        world: "data-collection-one-zero-zero",
        path: "wit/",
        async: true,
    });
}
