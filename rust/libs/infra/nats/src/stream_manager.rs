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
