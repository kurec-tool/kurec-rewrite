use serde::{Serialize, de::DeserializeOwned};

pub trait Event: Clone + Send + Sync + Sized + Serialize + DeserializeOwned + 'static {}
