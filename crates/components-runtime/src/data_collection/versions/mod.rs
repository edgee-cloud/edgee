use serde::Deserialize;

pub mod v1_0_0;
pub mod v1_0_1;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub enum DataCollectionWitVersion {
    #[serde(rename = "1.0.0")]
    #[default]
    V1_0_0,

    #[serde(rename = "1.0.1")]
    V1_0_1,
}
