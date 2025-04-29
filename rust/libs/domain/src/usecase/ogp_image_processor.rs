use crate::{
    error::DomainError,
    model::event::ogp::url::ImageRequest,
    ports::{ImageFetcher, ImageProcessor},
    repository::KvRepository,
};
use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebpImageData(pub Bytes);

impl From<Bytes> for WebpImageData {
    fn from(bytes: Bytes) -> Self {
        Self(bytes)
    }
}

impl From<WebpImageData> for Bytes {
    fn from(val: WebpImageData) -> Self {
        val.0
    }
}

#[async_trait]
pub trait OgpImageProcessorUseCase {
    async fn process_image_request(&self, request: &ImageRequest) -> Result<(), DomainError>;
}

pub struct OgpImageProcessorUseCaseImpl<F, P, R>
where
    F: ImageFetcher + Send + Sync,
    P: ImageProcessor + Send + Sync,
    R: KvRepository<String, WebpImageData> + Send + Sync,
{
    image_fetcher: F,
    image_processor: P,
    image_repository: R,
}

impl<F, P, R> OgpImageProcessorUseCaseImpl<F, P, R>
where
    F: ImageFetcher + Send + Sync,
    P: ImageProcessor + Send + Sync,
    R: KvRepository<String, WebpImageData> + Send + Sync,
{
    pub fn new(image_fetcher: F, image_processor: P, image_repository: R) -> Self {
        Self {
            image_fetcher,
            image_processor,
            image_repository,
        }
    }
}

#[async_trait]
impl<F, P, R> OgpImageProcessorUseCase for OgpImageProcessorUseCaseImpl<F, P, R>
where
    F: ImageFetcher + Send + Sync,
    P: ImageProcessor + Send + Sync,
    R: KvRepository<String, WebpImageData> + Send + Sync,
{
    async fn process_image_request(&self, request: &ImageRequest) -> Result<(), DomainError> {
        let url = &request.url;
        debug!("OGP画像URLを処理します: {}", url);

        let key = url.clone();

        let image_data = match self.image_fetcher.fetch_image(url).await {
            Ok(data) => data,
            Err(e) => {
                error!("画像の取得に失敗しました: {}", e);
                return Err(DomainError::ImageProcessingError(format!(
                    "画像の取得に失敗: {}",
                    e
                )));
            }
        };

        let webp_data = match self.image_processor.process_image(&image_data, 300).await {
            Ok(data) => data,
            Err(e) => {
                error!("画像の処理に失敗しました: {}", e);
                return Err(DomainError::ImageProcessingError(format!(
                    "画像の処理に失敗: {}",
                    e
                )));
            }
        };

        let webp_image_data = WebpImageData(Bytes::from(webp_data));
        match self
            .image_repository
            .put(key.clone(), &webp_image_data)
            .await
        {
            Ok(_) => {
                info!("WebP画像をKVSに保存しました: {}", key);
                Ok(())
            }
            Err(e) => {
                error!("WebP画像の保存に失敗しました: {}", e);
                Err(DomainError::ImageProcessingError(format!(
                    "WebP画像の保存に失敗: {}",
                    e
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ports::{ImageFetcherError, ImageProcessorError},
        repository::Versioned,
    };
    use async_trait::async_trait;
    use bytes::Bytes;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    struct MockImageFetcher {
        responses: HashMap<String, Result<Vec<u8>, ImageFetcherError>>,
    }

    impl MockImageFetcher {
        fn new() -> Self {
            Self {
                responses: HashMap::new(),
            }
        }

        fn mock_response(&mut self, url: &str, response: Result<Vec<u8>, ImageFetcherError>) {
            self.responses.insert(url.to_string(), response);
        }
    }

    #[async_trait]
    impl ImageFetcher for MockImageFetcher {
        async fn fetch_image(&self, url: &str) -> Result<Vec<u8>, ImageFetcherError> {
            self.responses
                .get(url)
                .cloned()
                .unwrap_or(Err(ImageFetcherError::FetchError(
                    "モックレスポンスが設定されていません".to_string(),
                )))
        }
    }

    struct MockImageProcessor {
        responses: HashMap<Vec<u8>, Result<Vec<u8>, ImageProcessorError>>,
    }

    impl MockImageProcessor {
        fn new() -> Self {
            Self {
                responses: HashMap::new(),
            }
        }

        fn mock_response(
            &mut self,
            input: Vec<u8>,
            response: Result<Vec<u8>, ImageProcessorError>,
        ) {
            self.responses.insert(input, response);
        }
    }

    #[async_trait]
    impl ImageProcessor for MockImageProcessor {
        async fn process_image(
            &self,
            image_data: &[u8],
            _width: u32,
        ) -> Result<Vec<u8>, ImageProcessorError> {
            self.responses.get(image_data).cloned().unwrap_or(Err(
                ImageProcessorError::ProcessError(
                    "モックレスポンスが設定されていません".to_string(),
                ),
            ))
        }
    }

    #[derive(Clone)]
    struct MockKvRepository {
        data: Arc<Mutex<HashMap<String, (u64, WebpImageData)>>>,
    }

    impl MockKvRepository {
        fn new() -> Self {
            Self {
                data: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl KvRepository<String, WebpImageData> for MockKvRepository {
        async fn put(&self, key: String, value: &WebpImageData) -> Result<(), DomainError> {
            let mut data = self.data.lock().unwrap();
            let revision = data.get(&key).map_or(1, |(rev, _)| rev + 1);
            data.insert(key, (revision, value.clone()));
            Ok(())
        }

        async fn get(&self, key: String) -> Result<Option<Versioned<WebpImageData>>, DomainError> {
            let data = self.data.lock().unwrap();
            Ok(data.get(&key).map(|(revision, value)| Versioned {
                revision: *revision,
                value: value.clone(),
            }))
        }

        async fn update(
            &self,
            key: String,
            value: &WebpImageData,
            revision: u64,
        ) -> Result<(), DomainError> {
            let mut data = self.data.lock().unwrap();
            if let Some((current_revision, _)) = data.get(&key) {
                if *current_revision != revision {
                    return Err(DomainError::ProgramsStoreError(
                        "リビジョンが一致しません".to_string(),
                    ));
                }
            } else {
                return Err(DomainError::ProgramsStoreError(
                    "キーが存在しません".to_string(),
                ));
            }
            data.insert(key, (revision + 1, value.clone()));
            Ok(())
        }

        async fn delete(&self, key: String) -> Result<(), DomainError> {
            let mut data = self.data.lock().unwrap();
            data.remove(&key);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_process_image_request_success() {
        let url = "https://example.com/image.jpg";
        let image_request = ImageRequest {
            url: url.to_string(),
        };
        let original_image = vec![1, 2, 3, 4, 5]; // 元の画像データ
        let processed_image = vec![10, 20, 30, 40, 50]; // 処理後の画像データ

        let mut image_fetcher = MockImageFetcher::new();
        image_fetcher.mock_response(url, Ok(original_image.clone()));

        let mut image_processor = MockImageProcessor::new();
        image_processor.mock_response(original_image.clone(), Ok(processed_image.clone()));

        let image_repository = MockKvRepository::new();

        let usecase = OgpImageProcessorUseCaseImpl::new(
            image_fetcher,
            image_processor,
            image_repository.clone(),
        );

        let result = usecase.process_image_request(&image_request).await;

        assert!(result.is_ok(), "処理が失敗しました: {:?}", result.err());

        let stored_data = image_repository
            .get(url.to_string())
            .await
            .unwrap()
            .expect("データが保存されていません");

        assert_eq!(
            stored_data.value.0.to_vec(),
            processed_image,
            "保存されたデータが一致しません"
        );
    }

    #[tokio::test]
    async fn test_process_image_request_fetch_error() {
        let url = "https://example.com/image.jpg";
        let image_request = ImageRequest {
            url: url.to_string(),
        };

        let mut image_fetcher = MockImageFetcher::new();
        image_fetcher.mock_response(
            url,
            Err(ImageFetcherError::FetchError("取得エラー".to_string())),
        );

        let image_processor = MockImageProcessor::new();
        let image_repository = MockKvRepository::new();

        let usecase = OgpImageProcessorUseCaseImpl::new(
            image_fetcher,
            image_processor,
            image_repository.clone(),
        );

        let result = usecase.process_image_request(&image_request).await;

        assert!(result.is_err(), "エラーが発生しませんでした");
        match result {
            Err(DomainError::ImageProcessingError(_)) => {}
            _ => panic!("期待されるエラータイプではありません: {:?}", result),
        }

        let stored_data = image_repository.get(url.to_string()).await.unwrap();
        assert!(stored_data.is_none(), "エラー時にデータが保存されています");
    }

    #[tokio::test]
    async fn test_process_image_request_process_error() {
        let url = "https://example.com/image.jpg";
        let image_request = ImageRequest {
            url: url.to_string(),
        };
        let original_image = vec![1, 2, 3, 4, 5]; // 元の画像データ

        let mut image_fetcher = MockImageFetcher::new();
        image_fetcher.mock_response(url, Ok(original_image.clone()));

        let mut image_processor = MockImageProcessor::new();
        image_processor.mock_response(
            original_image.clone(),
            Err(ImageProcessorError::ProcessError("処理エラー".to_string())),
        );

        let image_repository = MockKvRepository::new();

        let usecase = OgpImageProcessorUseCaseImpl::new(
            image_fetcher,
            image_processor,
            image_repository.clone(),
        );

        let result = usecase.process_image_request(&image_request).await;

        assert!(result.is_err(), "エラーが発生しませんでした");
        match result {
            Err(DomainError::ImageProcessingError(_)) => {}
            _ => panic!("期待されるエラータイプではありません: {:?}", result),
        }

        let stored_data = image_repository.get(url.to_string()).await.unwrap();
        assert!(stored_data.is_none(), "エラー時にデータが保存されています");
    }
}
