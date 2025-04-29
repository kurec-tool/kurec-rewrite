use async_trait::async_trait;
use domain::{
    error::DomainError,
    repository::{KvRepository, Versioned},
    usecase::WebpImageData,
};
use nats::{error::NatsInfraError, kvs::NatsKvRepositoryImpl, nats::NatsClient};

pub struct WebpImageDataRepository {
    inner: NatsKvRepositoryImpl<String, WebpImageData>,
}

impl WebpImageDataRepository {
    pub async fn new(nats_client: NatsClient) -> Result<Self, NatsInfraError> {
        let inner = NatsKvRepositoryImpl::new(nats_client).await?;
        Ok(Self { inner })
    }
}

#[async_trait]
impl KvRepository<String, WebpImageData> for WebpImageDataRepository {
    async fn put(&self, key: String, value: &WebpImageData) -> Result<(), DomainError> {
        self.inner.put(key, value).await
    }

    async fn get(&self, key: String) -> Result<Option<Versioned<WebpImageData>>, DomainError> {
        self.inner.get(key).await
    }

    async fn update(
        &self,
        key: String,
        value: &WebpImageData,
        revision: u64,
    ) -> Result<(), DomainError> {
        self.inner.update(key, value, revision).await
    }

    async fn delete(&self, key: String) -> Result<(), DomainError> {
        self.inner.delete(key).await
    }
}
