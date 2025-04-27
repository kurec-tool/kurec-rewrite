use std::vec;

use domain::model::recording::epg::EpgUpdated;
use futures::StreamExt as _;
use mirakc::get_mirakc_event_stream;
use nats::{
    nats::connect_nats,
    stream::EventStore,
    stream_manager::{StreamConfig, create_or_update_streams},
};
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let mut sse_stream = get_mirakc_event_stream("http://tuner:40772", 5)
        .await
        .unwrap();
    let nats_client = connect_nats("nats:4222").await.unwrap();
    let event_store = EventStore::<EpgUpdated>::new(nats_client.clone())
        .await
        .unwrap();

    let stream_config_list = vec![StreamConfig {
        name: "kurec".to_string(),
        subjects: vec!["recording.>".to_string()],
        ..Default::default()
    }];

    create_or_update_streams(&nats_client, &stream_config_list)
        .await
        .unwrap();

    while let Some(event) = sse_stream.next().await {
        debug!("Received event: {:?}", event);
        match event.event_type.as_str() {
            "epg.programs-updated" => {
                debug!("EPG updated: {:?}", event.data);
                let ev = serde_json::from_str::<mirakc::sse_event::ProgramsUpdated>(&event.data);
                match ev {
                    Ok(ev) => {
                        debug!("Parsed event: {:?}", ev);
                        let domain_ev = EpgUpdated {
                            service_id: ev.service_id,
                        };
                        event_store.publish_event(&domain_ev).await.unwrap();
                    }
                    Err(e) => {
                        debug!("Failed to parse event: {:?}", e);
                    }
                }
            }
            "recording.record-saved" => {
                debug!("Recording saved: {:?}", event.data);
            }
            _ => {
                debug!("Unknown event type: {:?}", event.event_type);
            }
        }
    }
}
