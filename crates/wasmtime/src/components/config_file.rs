use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

use serde::Deserialize;

use super::{ComponentsConfiguration, DataCollectionConfiguration};

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ComponentsConfigurationFile {
    pub data_collection: Vec<DataCollectionConfigurationFile>,
    pub cache: Option<PathBuf>,
}

impl ComponentsConfiguration for ComponentsConfigurationFile {
    fn get_collections(&self)->Vec<Arc<dyn DataCollectionConfiguration + Send + Sync>> {
        self.data_collection.clone().into_iter()
            .map(|e| Arc::new(e) as Arc<dyn DataCollectionConfiguration + Send + Sync>)
            .collect()
    }

    fn get_gache(&self)->Option<PathBuf> {
        self.cache.clone()
    }
}
#[derive(Deserialize, Debug, Default, Clone)]
pub struct DataCollectionConfigurationFile {
    pub name: String,
    pub component: String,
    pub credentials: HashMap<String, String>,
}

impl DataCollectionConfiguration for DataCollectionConfigurationFile {
    fn get_name(&self)->String {
        return self.name.clone()
    }

    fn get_wasm_binary(&self)->anyhow::Result<Vec<u8>> {
        let path = PathBuf::from(&self.component);
        fs::read(path).map_err(|e| anyhow::anyhow!("Error reading wasm binary at: {} error: {:?}",&self.component,e))
    }

    fn get_credentials(&self)->HashMap<String, String> {
        self.credentials.clone()
    }
}
