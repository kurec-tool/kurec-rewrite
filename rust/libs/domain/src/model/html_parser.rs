use crate::model::event::ogp;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HtmlParserError {
    #[error("HTMLの解析に失敗: {0}")]
    ParseError(String),
}

pub struct OgpImageParser;

impl OgpImageParser {
    pub fn extract_image_urls(html_content: &str) -> Result<Vec<String>, HtmlParserError> {
        match webpage::HTML::from_string(html_content.to_string(), None) {
            Ok(html) => {
                let mut image_urls = Vec::new();

                for image_obj in &html.opengraph.images {
                    image_urls.push(image_obj.url.clone());
                }

                Ok(image_urls)
            }
            Err(e) => Err(HtmlParserError::ParseError(e.to_string())),
        }
    }

    pub fn create_image_requests(
        html_content: &str,
    ) -> Result<Vec<ogp::url::ImageRequest>, HtmlParserError> {
        let image_urls = Self::extract_image_urls(html_content)?;

        let requests = image_urls
            .into_iter()
            .map(|url| ogp::url::ImageRequest { url })
            .collect();

        Ok(requests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_image_urls() {
        let html_content = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta property="og:image" content="https://example.com/image1.jpg" />
            <meta property="og:image" content="https://example.com/image2.png" />
        </head>
        <body>
            <p>Test content</p>
        </body>
        </html>
        "#;

        let result = OgpImageParser::extract_image_urls(html_content);
        assert!(result.is_ok());

        let image_urls = result.unwrap();
        assert_eq!(image_urls.len(), 2);
        assert!(image_urls.contains(&"https://example.com/image1.jpg".to_string()));
        assert!(image_urls.contains(&"https://example.com/image2.png".to_string()));
    }

    #[test]
    fn test_create_image_requests() {
        let html_content = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta property="og:image" content="https://example.com/image1.jpg" />
        </head>
        <body>
            <p>Test content</p>
        </body>
        </html>
        "#;

        let result = OgpImageParser::create_image_requests(html_content);
        assert!(result.is_ok());

        let requests = result.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url, "https://example.com/image1.jpg");
    }
}
