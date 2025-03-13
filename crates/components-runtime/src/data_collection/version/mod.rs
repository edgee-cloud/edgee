mod convert;
pub mod convert_0_5_0;
pub mod convert_1_0_0;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub enum DataCollectionProtocolVersion {
    V0_5_0,
    #[default]
    V1_0_0,
}

impl DataCollectionProtocolVersion {}
