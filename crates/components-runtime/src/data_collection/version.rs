use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub enum DataCollectionWitVersion {
    V0_5_0,
    #[default]
    V1_0_0,
}

impl DataCollectionWitVersion {}
