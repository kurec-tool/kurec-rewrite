use std::vec;

use clap::{Parser, Subcommand};
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

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// mirakcイベントを処理します
    Events {
        /// mirakcサーバーのURL
        #[arg(short, long, default_value = "http://tuner:40772")]
        mirakc_url: String,

        /// NATSサーバーのURL
        #[arg(short, long, default_value = "nats:4222")]
        nats_url: String,

        /// 再試行の最大回数（0は無限回）
        #[arg(short, long, default_value_t = 5)]
        retry_max: u32,
    },
    // 将来的に他のコマンドを追加する予定
}

#[tokio::main]
async fn main() {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Events {
            mirakc_url,
            nats_url,
            retry_max,
        } => {
            process_events(mirakc_url, nats_url, *retry_max).await;
        }
    }
}

async fn process_events(mirakc_url: &str, nats_url: &str, retry_max: u32) {
    let mut sse_stream = get_mirakc_event_stream(mirakc_url, retry_max)
        .await
        .unwrap();
    let nats_client = connect_nats(nats_url).await.unwrap();
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
