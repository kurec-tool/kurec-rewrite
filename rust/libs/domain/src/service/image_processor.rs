use crate::ports::{ImageProcessor, ImageProcessorError};
use async_trait::async_trait;
use image::GenericImageView;
use webp::Encoder;

#[derive(Default)]
pub struct WebpImageProcessor;

#[async_trait]
impl ImageProcessor for WebpImageProcessor {
    async fn process_image(
        &self,
        image_data: &[u8],
        width: u32,
    ) -> Result<Vec<u8>, ImageProcessorError> {
        let img = image::load_from_memory(image_data)
            .map_err(|e| ImageProcessorError::ProcessError(e.to_string()))?;

        let (orig_width, orig_height) = img.dimensions();

        let height = if orig_width > 0 {
            (orig_height as f32 * (width as f32 / orig_width as f32)) as u32
        } else {
            return Err(ImageProcessorError::ResizeError(
                "元の画像の幅が0です".to_string(),
            ));
        };

        let resized = img.resize(width, height, image::imageops::FilterType::Lanczos3);

        let encoder = Encoder::from_image(&resized)
            .map_err(|e| ImageProcessorError::ConversionError(e.to_string()))?;

        let webp_data = encoder.encode(80.0);

        Ok(webp_data.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use image::{ImageBuffer, Rgba};

    #[tokio::test]
    async fn test_process_image() {
        let width = 400;
        let height = 300;
        let mut img = ImageBuffer::new(width, height);

        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]);
        }

        let mut png_data = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .expect("Failed to write test image");

        let processor = WebpImageProcessor::default();

        let target_width = 300;
        let result = processor.process_image(&png_data, target_width).await;

        assert!(result.is_ok(), "画像処理に失敗: {:?}", result.err());

        let webp_data = result.unwrap();
        assert!(!webp_data.is_empty(), "WebPデータが空です");

        let webp_img = image::load_from_memory(&webp_data).expect("Failed to load WebP image");

        assert_eq!(
            webp_img.width(),
            target_width,
            "リサイズ後の幅が一致しません"
        );

        let expected_height = (height as f32 * (target_width as f32 / width as f32)) as u32;
        assert_eq!(
            webp_img.height(),
            expected_height,
            "リサイズ後の高さが一致しません"
        );
    }
}
