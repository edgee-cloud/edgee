use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub enum DataCollectionWitVersion {
    #[default]
    V1_0_0,
}

impl DataCollectionWitVersion {}
