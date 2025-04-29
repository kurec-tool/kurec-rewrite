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
    fn bucket_name() -> String {
        "programs_data".to_string()
    }

    async fn new(nats_client: NatsClient) -> Result<Self, NatsInfraError> {
        let inner =
            NatsKvRepositoryImpl::with_bucket_name(nats_client, Self::bucket_name()).await?;

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
    ($repo_name:ident, $key_type:ty, $value_type:ty, $bucket_name:expr) => {
        pub struct $repo_name {
            inner: $crate::kvs::NatsKvRepositoryImpl<$key_type, $value_type>,
        }

        #[async_trait::async_trait]
        impl $crate::kvs::NatsKvRepositoryTrait<$key_type, $value_type> for $repo_name {
            fn bucket_name() -> String {
                $bucket_name.to_string()
            }

            async fn new(
                nats_client: $crate::nats::NatsClient,
            ) -> Result<Self, $crate::error::NatsInfraError> {
                let inner = $crate::kvs::NatsKvRepositoryImpl::with_bucket_name(
                    nats_client,
                    Self::bucket_name(),
                )
                .await?;

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
