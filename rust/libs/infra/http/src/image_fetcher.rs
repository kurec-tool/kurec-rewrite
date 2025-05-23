use async_trait::async_trait;
use domain::ports::{ImageFetcher, ImageFetcherError};
use reqwest::Client;
use std::time::Duration;

pub struct ReqwestImageFetcher {
    client: Client,
}

impl Default for ReqwestImageFetcher {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }
}

impl ReqwestImageFetcher {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ImageFetcher for ReqwestImageFetcher {
    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>, ImageFetcherError> {
        self.client
            .get(url)
            .send()
            .await
            .map_err(|e| ImageFetcherError::FetchError(e.to_string()))?
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| ImageFetcherError::FetchError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::Filter;

    #[tokio::test]
    async fn test_fetch_image() {
        let mock_image_data = vec![1, 2, 3, 4, 5]; // テスト用の画像データ
        let mock_image_data_clone = mock_image_data.clone();

        let image_route = warp::path!("test-image.jpg").map(move || {
            let data = mock_image_data_clone.clone();
            warp::reply::with_header(data, "content-type", "image/jpeg")
        });

        let (addr, server) = warp::serve(image_route).bind_ephemeral(([127, 0, 0, 1], 0));

        let server_handle = tokio::spawn(server);

        let url = format!("http://127.0.0.1:{}/test-image.jpg", addr.port());

        let fetcher = ReqwestImageFetcher::default();

        let result = fetcher.fetch_image(&url).await;

        assert!(result.is_ok(), "画像の取得に失敗: {:?}", result.err());
        let image_data = result.unwrap();
        assert_eq!(image_data, mock_image_data);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_fetch_image_error() {
        let url = "http://non-existent-domain-12345.example";

        let fetcher = ReqwestImageFetcher::default();

        let result = fetcher.fetch_image(url).await;

        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                ImageFetcherError::FetchError(_) => {}
            }
        }
    }
}
