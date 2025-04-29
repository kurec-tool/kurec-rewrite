use async_trait::async_trait;
use domain::ports::{HtmlFetcher, HtmlFetcherError};
use reqwest::Client;
use tracing::error;

pub struct ReqwestHtmlFetcher {
    client: Client,
}

impl ReqwestHtmlFetcher {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl HtmlFetcher for ReqwestHtmlFetcher {
    async fn fetch_html(&self, url: &str) -> Result<String, HtmlFetcherError> {
        match self.client.get(url).send().await {
            Ok(response) => match response.text().await {
                Ok(html_content) => Ok(html_content),
                Err(e) => {
                    error!("レスポンスのテキスト取得に失敗: {:?}", e);
                    Err(HtmlFetcherError::FetchError(e.to_string()))
                }
            },
            Err(e) => {
                error!("URLの取得に失敗: {:?}", e);
                Err(HtmlFetcherError::FetchError(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_html_error() {
        let fetcher = ReqwestHtmlFetcher::new();
        let result = fetcher.fetch_html("invalid-url").await;
        assert!(result.is_err());
    }
}
