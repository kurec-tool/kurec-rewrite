use async_trait::async_trait;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum ImageFetcherError {
    #[error("画像URLの取得に失敗: {0}")]
    FetchError(String),
}

#[async_trait]
pub trait ImageFetcher {
    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>, ImageFetcherError>;
}
