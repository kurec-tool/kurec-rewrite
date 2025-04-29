use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HtmlFetcherError {
    #[error("URLの取得に失敗: {0}")]
    FetchError(String),
}

#[async_trait]
pub trait HtmlFetcher {
    async fn fetch_html(&self, url: &str) -> Result<String, HtmlFetcherError>;
}
