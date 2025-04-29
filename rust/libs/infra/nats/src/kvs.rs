use async_nats::jetstream;
use async_trait::async_trait;
use bytes::Bytes;
use domain::{
    error::DomainError,
    repository::{KvRepository, Versioned},
};
use tracing::{debug, error};

use crate::{error::NatsInfraError, nats::NatsClient};

pub struct NatsKvRepository {
    nats_client: NatsClient,
    bucket_name: String,
    kv_store: jetstream::kv::Store,
}

impl NatsKvRepository {
    pub async fn new(
        nats_client: NatsClient,
        bucket_name: String,
    ) -> Result<Self, NatsInfraError> {
        let js = nats_client.jetstream_context();
        let kv_store = match js.get_key_value(&bucket_name).await {
            Ok(store) => store,
            Err(_) => {
                js.create_key_value(jetstream::kv::Config {
                    bucket: bucket_name.clone(),
                    ..Default::default()
                })
                .await
                .map_err(|e| NatsInfraError::KvStore {
                    bucket_name: bucket_name.clone(),
                    source: Box::new(e),
                })?
            }
        };

        Ok(Self {
            nats_client,
            bucket_name,
            kv_store,
        })
    }

    async fn get_from_kv<K>(&self, key: &K) -> Result<Option<jetstream::kv::Entry>, NatsInfraError>
    where
        K: AsRef<str> + Send + Sync,
    {
        match self.kv_store.entry(key.as_ref()).await {
            Ok(Some(entry)) if entry.value.is_empty() => {
                debug!(
                    bucket = %self.bucket_name,
                    key = %key.as_ref(),
                    revision = %entry.revision,
                    "空の値を持つエントリを削除済みとして扱います"
                );
                Ok(None)
            },
            Ok(entry) => Ok(entry),
            Err(e) => Err(NatsInfraError::KvGet { source: Box::new(e) }),
        }
    }
}

#[async_trait]
impl<K, V> KvRepository<K, V> for NatsKvRepository
where
    K: AsRef<str> + Send + Sync + 'static,
    V: Into<Bytes> + From<Bytes> + Send + Sync + Clone + 'static,
{
    async fn put(&self, key: K, value: &V) -> Result<(), DomainError> {
        let value_clone = value.clone().into();
        debug!(
            bucket = %self.bucket_name,
            key = %key.as_ref(),
            "KVバケットに値を保存します"
        );
        self.kv_store
            .put(key.as_ref(), value_clone)
            .await
            .map_err(|e| {
                error!(
                    bucket = %self.bucket_name,
                    key = %key.as_ref(),
                    error = %e,
                    "KVバケットへの値の保存に失敗しました"
                );
                DomainError::ProgramsStoreError(format!("KVSへの保存エラー: {}", e))
            })?;
        Ok(())
    }

    async fn get(&self, key: K) -> Result<Option<Versioned<V>>, DomainError> {
        debug!(
            bucket = %self.bucket_name,
            key = %key.as_ref(),
            "KVバケットから値を取得します"
        );
        let entry = match self.get_from_kv(&key).await {
            Ok(Some(entry)) => entry,
            Ok(None) => return Ok(None),
            Err(e) => {
                error!(
                    bucket = %self.bucket_name,
                    key = %key.as_ref(),
                    error = %e,
                    "KVバケットからの値の取得に失敗しました"
                );
                return Err(DomainError::ProgramsRetrievalError(format!(
                    "KVSからの取得エラー: {}",
                    e
                )));
            }
        };

        let bytes_value = entry.value;
        let value: V = V::from(bytes_value);
        let versioned = Versioned {
            revision: entry.revision,
            value,
        };
        Ok(Some(versioned))
    }

    async fn update(&self, key: K, value: &V, revision: u64) -> Result<(), DomainError> {
        let value_clone = value.clone().into();
        debug!(
            bucket = %self.bucket_name,
            key = %key.as_ref(),
            revision = %revision,
            "KVバケットの値を更新します"
        );
        self.kv_store
            .update(key.as_ref(), value_clone, revision)
            .await
            .map_err(|e| {
                error!(
                    bucket = %self.bucket_name,
                    key = %key.as_ref(),
                    revision = %revision,
                    error = %e,
                    "KVバケットの値の更新に失敗しました"
                );
                DomainError::ProgramsStoreError(format!("KVSの更新エラー: {}", e))
            })?;
        Ok(())
    }

    async fn delete(&self, key: K) -> Result<(), DomainError> {
        debug!(
            bucket = %self.bucket_name,
            key = %key.as_ref(),
            "KVバケットから値を削除します"
        );
        self.kv_store.delete(key.as_ref()).await.map_err(|e| {
            error!(
                bucket = %self.bucket_name,
                key = %key.as_ref(),
                error = %e,
                "KVバケットからの値の削除に失敗しました"
            );
            DomainError::ProgramsStoreError(format!("KVSの削除エラー: {}", e))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{nats::connect_nats, test_util::setup_toxi_proxy_nats};
    use bytes::Bytes;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_nats_kv_repository_create() {
        let proxy_nats = setup_toxi_proxy_nats().await.unwrap();
        let nats_client = connect_nats(&proxy_nats.nats_url).await.unwrap();
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let bucket_name = format!("test_bucket_{}", timestamp);
        
        let repo = NatsKvRepository::new(nats_client, bucket_name).await.unwrap();
        assert_eq!(repo.kv_store.status().await.unwrap().bucket(), &repo.bucket_name);
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let proxy_nats = setup_toxi_proxy_nats().await.unwrap();
        let nats_client = connect_nats(&proxy_nats.nats_url).await.unwrap();
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let bucket_name = format!("test_bucket_{}", timestamp);
        
        let repo = NatsKvRepository::new(nats_client, bucket_name).await.unwrap();
        
        let key = "test_key";
        let value = Bytes::from("test_value");
        
        repo.put(key, &value).await.unwrap();
        
        let result: Option<Versioned<Bytes>> = repo.get(key).await.unwrap();
        assert!(result.is_some());
        
        let versioned: Versioned<Bytes> = result.unwrap();
        assert_eq!(versioned.value, value);
        assert_eq!(versioned.revision, 1); // 最初のリビジョンは1
    }

    #[tokio::test]
    async fn test_update() {
        let proxy_nats = setup_toxi_proxy_nats().await.unwrap();
        let nats_client = connect_nats(&proxy_nats.nats_url).await.unwrap();
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let bucket_name = format!("test_bucket_{}", timestamp);
        
        let repo = NatsKvRepository::new(nats_client, bucket_name).await.unwrap();
        
        let key = "test_key";
        let value1 = Bytes::from("initial_value");
        let value2 = Bytes::from("updated_value");
        
        repo.put(key, &value1).await.unwrap();
        
        let result: Versioned<Bytes> = repo.get(key).await.unwrap().unwrap();
        assert_eq!(result.value, value1);
        let revision = result.revision;
        
        repo.update(key, &value2, revision).await.unwrap();
        
        let updated: Versioned<Bytes> = repo.get(key).await.unwrap().unwrap();
        assert_eq!(updated.value, value2);
        assert_eq!(updated.revision, revision + 1);
    }

    #[tokio::test]
    async fn test_delete() {
        use tokio::time::sleep;
        use std::time::Duration;
        use tracing::info;
        
        crate::test_util::init_test_logging();
        
        let proxy_nats = setup_toxi_proxy_nats().await.unwrap();
        let nats_client = connect_nats(&proxy_nats.nats_url).await.unwrap();
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let bucket_name = format!("test_bucket_delete_{}", timestamp);
        
        let repo = NatsKvRepository::new(nats_client, bucket_name).await.unwrap();
        
        let key = "test_key_delete";
        let value = Bytes::from("test_value");
        
        info!("値を設定します: key={}", key);
        repo.put(key, &value).await.unwrap();
        
        info!("値が存在することを確認します: key={}", key);
        let result: Option<Versioned<Bytes>> = repo.get(key).await.unwrap();
        assert!(result.is_some(), "値が正しく保存されていません");
        
        info!("値を削除します: key={}", key);
        <NatsKvRepository as KvRepository<&str, Bytes>>::delete(&repo, key).await.unwrap();
        
        info!("削除後に待機します: {}秒", 3);
        sleep(Duration::from_secs(3)).await;
        
        info!("値が存在しないことを確認します: key={}", key);
        let deleted: Option<Versioned<Bytes>> = repo.get(key).await.unwrap();
        
        if deleted.is_some() {
            let entry = deleted.unwrap();
            panic!("キーが削除されていません。revision={}, value={:?}", 
                   entry.revision, 
                   String::from_utf8_lossy(&entry.value.to_vec()));
        }
    }

    #[tokio::test]
    async fn test_update_non_existent_key() {
        let proxy_nats = setup_toxi_proxy_nats().await.unwrap();
        let nats_client = connect_nats(&proxy_nats.nats_url).await.unwrap();
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let bucket_name = format!("test_bucket_{}", timestamp);
        
        let repo = NatsKvRepository::new(nats_client, bucket_name).await.unwrap();
        
        let key = "non_existent_key";
        let value = Bytes::from("test_value");
        let result = repo.update(key, &value, 1).await;
        
        assert!(result.is_err());
    }
}
