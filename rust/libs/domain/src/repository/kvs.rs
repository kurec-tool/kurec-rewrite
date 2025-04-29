use crate::error::DomainError;
use async_trait::async_trait;
use bytes::Bytes;

pub struct Versioned<V>
where
    V: Into<Bytes> + Send + Sync,
{
    pub revision: u64,
    pub value: V,
}

#[async_trait]
pub trait KvRepository<K, V>
where
    K: AsRef<str> + Send + Sync,
    V: Into<Bytes> + Send + Sync,
{
    async fn put(&self, key: K, value: &V) -> Result<(), DomainError>;
    async fn get(&self, key: K) -> Result<Option<Versioned<V>>, DomainError>;
    async fn update(&self, key: K, value: &V, revision: u64) -> Result<(), DomainError>;
    async fn delete(&self, key: K) -> Result<(), DomainError>;
}
