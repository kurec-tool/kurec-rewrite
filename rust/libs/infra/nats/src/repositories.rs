use async_trait::async_trait;
use domain::model::program::ProgramsData;
use domain::repository::KvRepository;

use crate::error::NatsInfraError;
use crate::kvs::{NatsKvRepositoryImpl, NatsKvRepositoryTrait};
use crate::nats::NatsClient;

pub struct ProgramsDataRepository {
    inner: NatsKvRepositoryImpl<String, ProgramsData>,
}

#[async_trait]
impl NatsKvRepositoryTrait<String, ProgramsData> for ProgramsDataRepository {
    async fn new(nats_client: NatsClient) -> Result<Self, NatsInfraError> {
        let inner = NatsKvRepositoryImpl::new(nats_client).await?;

        Ok(Self { inner })
    }
}

#[async_trait]
impl KvRepository<String, ProgramsData> for ProgramsDataRepository {
    async fn put(
        &self,
        key: String,
        value: &ProgramsData,
    ) -> Result<(), domain::error::DomainError> {
        self.inner.put(key, value).await
    }

    async fn get(
        &self,
        key: String,
    ) -> Result<Option<domain::repository::Versioned<ProgramsData>>, domain::error::DomainError>
    {
        self.inner.get(key).await
    }

    async fn update(
        &self,
        key: String,
        value: &ProgramsData,
        revision: u64,
    ) -> Result<(), domain::error::DomainError> {
        self.inner.update(key, value, revision).await
    }

    async fn delete(&self, key: String) -> Result<(), domain::error::DomainError> {
        self.inner.delete(key).await
    }
}

#[macro_export]
macro_rules! define_repository {
    ($repo_name:ident, $key_type:ty, $value_type:ty) => {
        pub struct $repo_name {
            inner: $crate::kvs::NatsKvRepositoryImpl<$key_type, $value_type>,
        }

        #[async_trait::async_trait]
        impl $crate::kvs::NatsKvRepositoryTrait<$key_type, $value_type> for $repo_name {
            async fn new(
                nats_client: $crate::nats::NatsClient,
            ) -> Result<Self, $crate::error::NatsInfraError> {
                let inner = $crate::kvs::NatsKvRepositoryImpl::new(nats_client).await?;

                Ok(Self { inner })
            }
        }

        #[async_trait::async_trait]
        impl domain::repository::KvRepository<$key_type, $value_type> for $repo_name {
            async fn put(
                &self,
                key: $key_type,
                value: &$value_type,
            ) -> Result<(), domain::error::DomainError> {
                self.inner.put(key, value).await
            }

            async fn get(
                &self,
                key: $key_type,
            ) -> Result<
                Option<domain::repository::Versioned<$value_type>>,
                domain::error::DomainError,
            > {
                self.inner.get(key).await
            }

            async fn update(
                &self,
                key: $key_type,
                value: &$value_type,
                revision: u64,
            ) -> Result<(), domain::error::DomainError> {
                self.inner.update(key, value, revision).await
            }

            async fn delete(&self, key: $key_type) -> Result<(), domain::error::DomainError> {
                self.inner.delete(key).await
            }
        }
    };
}

#[cfg(test)]
pub mod test {
    use crate::define_repository;
    use crate::error::NatsInfraError;
    use crate::kvs::NatsKvRepositoryImpl;
    use bytes::Bytes;

    #[derive(Clone, Debug, PartialEq)]
    pub struct TestData(pub Bytes);

    impl From<Bytes> for TestData {
        fn from(bytes: Bytes) -> Self {
            TestData(bytes)
        }
    }

    impl Into<Bytes> for TestData {
        fn into(self) -> Bytes {
            self.0
        }
    }

    define_repository!(TestDataRepository, String, TestData);

    impl TestDataRepository {
        pub fn get_bucket_name(&self) -> &str {
            &self.inner.bucket_name
        }

        pub async fn get_kv_store_status(
            &self,
        ) -> Result<async_nats::jetstream::kv::bucket::Status, crate::error::NatsInfraError>
        {
            self.inner
                .kv_store
                .status()
                .await
                .map_err(|e| crate::error::NatsInfraError::KvGet {
                    source: Box::new(e),
                })
        }
    }
}
