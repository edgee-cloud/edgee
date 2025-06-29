use serde::Deserialize;

pub mod v1_0_0;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub enum EdgeFunctionWitVersion {
    #[serde(rename = "1.0.0")]
    #[default]
    V1_0_0,
}
