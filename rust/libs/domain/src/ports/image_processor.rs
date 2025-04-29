use async_trait::async_trait;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum ImageProcessorError {
    #[error("画像の処理に失敗: {0}")]
    ProcessError(String),

    #[error("画像のリサイズに失敗: {0}")]
    ResizeError(String),

    #[error("WebP形式への変換に失敗: {0}")]
    ConversionError(String),
}

#[async_trait]
pub trait ImageProcessor {
    async fn process_image(
        &self,
        image_data: &[u8],
        width: u32,
    ) -> Result<Vec<u8>, ImageProcessorError>;
}
