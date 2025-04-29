use linkify::{LinkFinder, LinkKind};
use std::collections::HashSet;
use url::Url;

pub struct UrlExtractor {
    excluded_domains: HashSet<String>,
}

impl UrlExtractor {
    pub fn new(excluded_domains: Vec<String>) -> Self {
        let excluded_domains = excluded_domains.into_iter().collect();
        Self { excluded_domains }
    }

    pub fn with_default_exclusions() -> Self {
        Self::new(vec![
            "x.com".to_string(),
            "twitter.com".to_string(),
            "facebook.com".to_string(),
            "tiktok.com".to_string(),
            "instagram.com".to_string(),
        ])
    }

    pub fn extract_urls(&self, text: &str) -> Vec<String> {
        let mut finder = LinkFinder::new();
        finder.kinds(&[LinkKind::Url]);

        finder
            .links(text)
            .filter_map(|link| {
                let url = link.as_str().to_string();
                if let Ok(url_parsed) = Url::parse(&url) {
                    if let Some(host) = url_parsed.host_str() {
                        if !self.is_excluded_domain(host) {
                            return Some(url);
                        }
                    }
                }
                None
            })
            .collect()
    }

    fn is_excluded_domain(&self, host: &str) -> bool {
        self.excluded_domains
            .iter()
            .any(|domain| host == domain || host.ends_with(&format!(".{}", domain)))
    }
}

impl Default for UrlExtractor {
    fn default() -> Self {
        Self::with_default_exclusions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_urls() {
        let extractor = UrlExtractor::default();

        let text =
            "これはテストです。https://example.com と http://test.org にアクセスしてください。";
        let urls = extractor.extract_urls(text);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://example.com".to_string()));
        assert!(urls.contains(&"http://test.org".to_string()));

        let text = "これはTwitter https://twitter.com/user やFacebook https://facebook.com/user のURLです。";
        let urls = extractor.extract_urls(text);
        assert_eq!(urls.len(), 0);

        let text = "これはTwitter https://mobile.twitter.com/user のURLです。";
        let urls = extractor.extract_urls(text);
        assert_eq!(urls.len(), 0);

        let text = "これはhttps://example.com とhttps://x.com のURLです。";
        let urls = extractor.extract_urls(text);
        assert_eq!(urls.len(), 1);
        assert!(urls.contains(&"https://example.com".to_string()));

        let text = "詳細は http://example.com/long/path/to/url/index.html?param=value#section を参照してください。";
        let urls = extractor.extract_urls(text);
        assert_eq!(urls.len(), 1);
        assert!(urls.contains(
            &"http://example.com/long/path/to/url/index.html?param=value#section".to_string()
        ));
    }
}
