use std::collections::HashMap;
use tracing::warn;

pub fn convert_properties(properties: HashMap<String, serde_json::Value>) -> Vec<(String, String)> {
    use serde_json::Value;

    if properties.is_empty() {
        return Vec::new();
    };

    properties
        .into_iter()
        .filter(|(_, value)| !(value.is_array() || value.is_object()))
        .map(|(k, v)| {
            let value = if let Value::String(s) = v {
                s
            } else {
                v.to_string()
            };

            (k, value)
        })
        .collect()
}

pub fn convert_products(
    properties: HashMap<String, serde_json::Value>,
) -> Vec<Vec<(String, String)>> {
    use serde_json::Value;

    if properties.is_empty() {
        return Vec::new();
    };

    // if the key is products, then we need to convert the value to a list of tuples
    if let Some(products) = properties.get("products") {
        // if products is not an array, return an empty vector
        if !products.is_array() {
            warn!("data.properties.products is not an array, skipping");
            return Vec::new();
        }

        let mut results: Vec<Vec<(String, String)>> = Vec::new();
        let items = products.as_array().unwrap();
        items.iter().enumerate().for_each(|(index, product)| {
            // if product is not an object, go to the next product
            if !product.is_object() {
                warn!(
                    "data.properties.products[{}] is not an object, skipping",
                    index
                );
                return;
            }

            let mut i: Vec<(String, String)> = Vec::new();
            let dict = product.as_object().unwrap().clone();
            dict.into_iter()
                .filter(|(_, value)| !(value.is_array() || value.is_object()))
                .map(|(k, v)| {
                    let value = if let Value::String(s) = v {
                        s
                    } else {
                        v.to_string()
                    };
                    (k, value)
                })
                .for_each(|tuple| i.push(tuple));

            results.push(i);
        });
        results
    } else {
        Vec::new()
    }
}
