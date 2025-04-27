use std::{any::type_name, time::Duration};

use domain::types::Event;
use tracing::debug;

use crate::{
    error::NatsInfraError,
    nats::{NatsClient, connect_nats},
};

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

    fn get_client(&self) -> &NatsClient {
        &self.nats_client
    }

    fn get_subject() -> String {
        let event_type_name = type_name::<E>();
        let mut segments = event_type_name
            .rsplit("::")
            .map(|s| heck::ToSnakeCase::to_snake_case(s));
        let event_name = segments.next().unwrap_or("unknown_event".to_string());
        let resource_name = segments.next().unwrap_or("unknown_resource".to_string());
        let domain_name = segments.next().unwrap_or("unknown_domain".to_string());
        format!("{domain_name}.{resource_name}.{event_name}")
    }

    pub async fn publish_event(&self, event: E) -> Result<(), NatsInfraError> {
        let subject = Self::get_subject();

        debug!("Publishing event on subject: {}", &subject);
        let js = self.nats_client.jetstream_context();
        let payload = serde_json::to_vec(&event).map_err(|e| NatsInfraError::Json {
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
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use crate::{nats, test_util::setup_toxi_proxy_nats};

    use super::*;
    use futures::StreamExt;

    pub mod test_domain {
        pub mod test_resource {
            use serde::{Deserialize, Serialize};

            #[derive(Clone, Debug, Deserialize, Serialize)]
            pub struct TestEvent;
        }
    }
    use test_domain::test_resource::TestEvent;

    impl Event for TestEvent {}

    #[derive(Clone)]
    struct TestStream;

    type TestEventStore = EventStore<TestEvent>;

    #[test]
    fn test_event_stream_names() {
        let subject = TestEventStore::get_subject();
        assert_eq!(subject, "test_domain.test_resource.test_event");
    }

    #[tokio::test]
    async fn test_publish_event() {
        let mut proxy_nats = setup_toxi_proxy_nats().await.unwrap();

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

        let event = TestEvent;
        event_stream.publish_event(event).await.unwrap();

        let mut consumer = stream
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
        assert_eq!(msg.payload, serde_json::to_vec(&TestEvent).unwrap());

        proxy_nats.cleanup().await.unwrap();
    }
}
