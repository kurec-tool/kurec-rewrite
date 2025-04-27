use serde::{Serialize, de::DeserializeOwned};

pub trait Event: Clone + Send + Sync + Serialize + DeserializeOwned + 'static {}
