use std::any::type_name;

use async_nats::jetstream::consumer::PullConsumer;
use domain::types::Event;
use futures::{StreamExt, TryStreamExt};
use tracing::debug;

use crate::{error::NatsInfraError, nats::NatsClient};

pub trait EventReader<E: Event> {
    fn next(&self) -> impl std::future::Future<Output = Result<E, NatsInfraError>> + Send;
}

pub struct EventStoreReader<E: Event> {
    subject: String,
    consumer: PullConsumer,
    _phantom: std::marker::PhantomData<E>,
}

impl<E: Event> EventReader<E> for EventStoreReader<E> {
    async fn next(&self) -> Result<E, NatsInfraError> {
        loop {
            let result = self
                .consumer
                .fetch()
                .max_messages(1)
                .messages()
                .await
                .map_err(|e| NatsInfraError::StreamRetrieval {
                    stream_name: "unknown".to_string(),
                    source: Box::new(e),
                })?
                .into_stream()
                .next()
                .await;
            match result {
                Some(Ok(msg)) => {
                    let ev: E = serde_json::from_slice(&msg.payload).map_err(|e| {
                        NatsInfraError::JsonDeserialize {
                            subject: self.subject.clone(),
                            message: msg.payload.clone().into(),
                            source: e,
                        }
                    })?;
                    return Ok(ev);
                }
                Some(Err(e)) => {
                    return Err(NatsInfraError::StreamRetrieval {
                        stream_name: "unknown".to_string(),
                        source: e,
                    });
                }
                None => {
                    debug!("メッセージが届いていないので再試行します...")
                }
            }
        }
    }
}

pub struct EventStore<E: Event> {
    nats_client: NatsClient,
    _phantom: std::marker::PhantomData<E>,
}

impl<E: Event> EventStore<E> {
    pub async fn new(nats_client: NatsClient) -> Result<Self, NatsInfraError> {
        Ok(Self {
            nats_client,
            _phantom: std::marker::PhantomData,
        })
    }

    #[cfg(test)]
    fn get_client(&self) -> &NatsClient {
        &self.nats_client
    }

    pub fn get_subject() -> String {
        let event_type_name = type_name::<E>();
        let mut segments = event_type_name
            .rsplit("::")
            .map(heck::ToSnakeCase::to_snake_case);
        let event_name = segments.next().unwrap_or("unknown_event".to_string());
        let resource_name = segments.next().unwrap_or("unknown_resource".to_string());
        let domain_name = segments.next().unwrap_or("unknown_domain".to_string());
        format!("{domain_name}.{resource_name}.{event_name}")
    }

    pub async fn publish_event(&self, event: &E) -> Result<(), NatsInfraError> {
        let subject = Self::get_subject();

        debug!("Publishing event on subject: {}", &subject);
        let js = self.nats_client.jetstream_context();
        let payload = serde_json::to_vec(&event).map_err(|e| NatsInfraError::JsonSerialize {
            subject: subject.clone(),
            source: e,
        })?;
        js.publish(subject.clone(), payload.into())
            .await
            .map_err(|e| NatsInfraError::EventPublish {
                subject: subject.clone(),
                source: Box::new(e),
            })?
            .await
            .map_err(|e| NatsInfraError::EventPublish {
                subject: subject.clone(),
                source: Box::new(e),
            })?;
        Ok(())
    }

    pub async fn get_reader(
        &self,
        durable_name: String,
    ) -> Result<impl EventReader<E>, NatsInfraError> {
        let subject = Self::get_subject();
        let js = self.nats_client.jetstream_context();
        let stream = js
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: "test-stream".to_string(),
                subjects: vec![subject.clone()],
                ..Default::default()
            })
            .await
            .map_err(|e| NatsInfraError::StreamCreation {
                stream_name: subject.clone(),
                source: Box::new(e),
            })?;

        let consumer = stream
            .create_consumer(async_nats::jetstream::consumer::pull::Config {
                filter_subject: subject.clone(),
                durable_name: Some(durable_name),
                ..Default::default()
            })
            .await
            .map_err(|e| NatsInfraError::StreamRetrieval {
                stream_name: subject.clone(),
                source: Box::new(e),
            })?;

        Ok(EventStoreReader {
            subject,
            consumer,
            _phantom: std::marker::PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{nats::connect_nats, test_util::setup_toxi_proxy_nats};

    use super::*;
    use futures::StreamExt;

    pub mod test_domain {
        pub mod test_resource {
            use serde::{Deserialize, Serialize};

            #[derive(Clone, Debug, Deserialize, Serialize)]
            pub struct TestEvent {
                pub data: String,
            }
        }
    }
    use test_domain::test_resource::TestEvent;

    impl Event for TestEvent {}

    type TestEventStore = EventStore<TestEvent>;

    #[test]
    fn test_event_stream_names() {
        let subject = TestEventStore::get_subject();
        assert_eq!(subject, "test_domain.test_resource.test_event");
    }

    #[tokio::test]
    async fn test_publish_event() {
        let proxy_nats = setup_toxi_proxy_nats().await.unwrap();

        let nats_url = &proxy_nats.nats_url;
        let nats_client = connect_nats(nats_url).await.unwrap();
        let event_stream = TestEventStore::new(nats_client).await.unwrap();

        // ストリームを作成しておく
        let js = event_stream.get_client().jetstream_context();
        let stream = js
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: "test-stream".to_string(),
                subjects: vec![TestEventStore::get_subject()],
                ..Default::default()
            })
            .await
            .unwrap();

        let event = TestEvent {
            data: "test data".to_string(),
        };
        event_stream.publish_event(&event).await.unwrap();

        let consumer = stream
            .create_consumer(async_nats::jetstream::consumer::pull::Config {
                filter_subject: TestEventStore::get_subject(),
                durable_name: Some("test_consumer".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();
        let mut messages = consumer.messages().await.unwrap();

        let msg = messages.next().await.unwrap().unwrap();

        assert_eq!(msg.subject.as_str(), "test_domain.test_resource.test_event");
        assert_eq!(msg.payload, serde_json::to_vec(&event).unwrap());
    }

    #[tokio::test]
    async fn test_get_reader() {
        let proxy_nats = setup_toxi_proxy_nats().await.unwrap();

        let nats_url = &proxy_nats.nats_url;
        let nats_client = connect_nats(nats_url).await.unwrap();
        let event_stream = TestEventStore::new(nats_client).await.unwrap();

        // ストリームを作成しておく
        let js = event_stream.get_client().jetstream_context();
        let _stream = js
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: "test-stream".to_string(),
                subjects: vec![TestEventStore::get_subject()],
                ..Default::default()
            })
            .await
            .unwrap();

        // イベントを発行
        let event = TestEvent {
            data: "test data".to_string(),
        };
        event_stream.publish_event(&event).await.unwrap();

        let durable_name = "test_consumer".to_string();
        let reader = event_stream.get_reader(durable_name.clone()).await.unwrap();
        let ev = reader.next().await.unwrap();
        assert_eq!(ev.data, event.data);
    }
}
