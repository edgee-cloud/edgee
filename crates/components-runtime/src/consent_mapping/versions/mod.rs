use serde::Deserialize;

pub mod v1_0_0;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub enum ConsentMappingWitVersion {
    #[default]
    V1_0_0,
}
