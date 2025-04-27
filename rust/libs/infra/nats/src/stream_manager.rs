use crate::{error::NatsInfraError, nats::NatsClient};
pub use async_nats::jetstream::stream::Config as StreamConfig;

pub async fn create_or_update_streams(
    nats_client: &NatsClient,
    stream_config_list: &[async_nats::jetstream::stream::Config],
) -> Result<(), NatsInfraError> {
    let js = nats_client.jetstream_context();
    for config in stream_config_list {
        js.get_or_create_stream(config.clone()).await.map_err(|e| {
            NatsInfraError::StreamCreation {
                stream_name: config.name.clone(),
                source: Box::new(e),
            }
        })?;
        js.update_stream(config)
            .await
            .map_err(|e| NatsInfraError::StreamCreation {
                stream_name: config.name.clone(),
                source: Box::new(e),
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{PROXY_NAME, disable_proxy, enable_proxy};
    use crate::{nats::connect_nats, test_util::setup_toxi_proxy_nats};
    use reqwest::Client as HttpClient;

    #[tokio::test]
    async fn test_create_or_update_streams_success() {
        let mut proxy_nats = setup_toxi_proxy_nats().await.unwrap();
        let nats_url = &proxy_nats.nats_url;
        let nats_client = connect_nats(nats_url).await.unwrap();

        let stream_configs = vec![
            async_nats::jetstream::stream::Config {
                name: "test-stream-1".to_string(),
                subjects: vec!["test.subject.1".to_string()],
                ..Default::default()
            },
            async_nats::jetstream::stream::Config {
                name: "test-stream-2".to_string(),
                subjects: vec!["test.subject.2".to_string()],
                ..Default::default()
            },
        ];

        let result = create_or_update_streams(&nats_client, &stream_configs).await;
        assert!(result.is_ok());

        let js = nats_client.jetstream_context();
        for config in &stream_configs {
            let stream = js.get_stream(&config.name).await;
            assert!(stream.is_ok());
        }

        proxy_nats.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn test_create_or_update_streams_network_failure() {
        let mut proxy_nats = setup_toxi_proxy_nats().await.unwrap();
        let nats_url = &proxy_nats.nats_url;
        let nats_client = connect_nats(nats_url).await.unwrap();

        let stream_configs = vec![async_nats::jetstream::stream::Config {
            name: "test-failure-stream".to_string(),
            subjects: vec!["test.failure.subject".to_string()],
            ..Default::default()
        }];

        let http_client = HttpClient::new();
        disable_proxy(&http_client, &proxy_nats.api_url, PROXY_NAME)
            .await
            .unwrap();

        let result = create_or_update_streams(&nats_client, &stream_configs).await;
        assert!(result.is_err());

        if let Err(err) = result {
            match err {
                NatsInfraError::StreamCreation { .. } => {}
                _ => {
                    panic!("期待したエラー型ではありません: {:?}", err);
                }
            }
        }

        enable_proxy(&http_client, &proxy_nats.api_url, PROXY_NAME)
            .await
            .unwrap();

        proxy_nats.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn test_create_or_update_streams_invalid_config() {
        let mut proxy_nats = setup_toxi_proxy_nats().await.unwrap();
        let nats_url = &proxy_nats.nats_url;
        let nats_client = connect_nats(nats_url).await.unwrap();

        let stream_configs = vec![async_nats::jetstream::stream::Config {
            name: "".to_string(), // 空の名前は無効
            subjects: vec!["test.invalid.subject".to_string()],
            ..Default::default()
        }];

        let result = create_or_update_streams(&nats_client, &stream_configs).await;
        assert!(result.is_err());

        if let Err(err) = result {
            match err {
                NatsInfraError::StreamCreation { .. } => {}
                _ => {
                    panic!("期待したエラー型ではありません: {:?}", err);
                }
            }
        }

        proxy_nats.cleanup().await.unwrap();
    }
}
