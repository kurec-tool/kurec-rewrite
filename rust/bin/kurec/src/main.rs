use std::vec;

use clap::{Parser, Subcommand};
use domain::model::event::recording::epg::Updated;
use domain::ports::ProgramsRetriever;
use futures::StreamExt as _;
use mirakc::get_mirakc_event_stream;
use nats::{
    nats::connect_nats,
    stream::{EventReader, EventStore},
    stream_manager::{StreamConfig, create_or_update_streams},
};
use tracing::{debug, error};
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
    EpgRetriever {
        /// mirakcサーバーのURL
        #[arg(short, long, default_value = "http://tuner:40772")]
        mirakc_url: String,

        /// NATSサーバーのURL
        #[arg(short, long, default_value = "nats:4222")]
        nats_url: String,
    },
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
        Commands::EpgRetriever {
            mirakc_url,
            nats_url,
        } => {
            process_epg_retriever(mirakc_url, nats_url).await;
        }
    }
}

async fn setup_kurec_streams(
    nats_client: &nats::nats::NatsClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let kurec_stream_config = StreamConfig {
        name: "kurec".to_string(),
        subjects: vec![
            "recording.>".to_string(), // すべてのrecordingイベントをカバー
        ],
        ..Default::default()
    };

    let stream_configs = vec![kurec_stream_config];

    create_or_update_streams(nats_client, &stream_configs).await?;

    Ok(())
}

async fn process_events(mirakc_url: &str, nats_url: &str, retry_max: u32) {
    let mut sse_stream = get_mirakc_event_stream(mirakc_url, retry_max)
        .await
        .unwrap();
    let nats_client = connect_nats(nats_url).await.unwrap();
    let event_store = EventStore::<Updated>::new(nats_client.clone())
        .await
        .unwrap();

    setup_kurec_streams(&nats_client).await.unwrap();

    while let Some(event) = sse_stream.next().await {
        debug!("Received event: {:?}", event);
        match event.event_type.as_str() {
            "epg.programs-updated" => {
                debug!("EPG updated: {:?}", event.data);
                let ev = serde_json::from_str::<mirakc::sse_event::ProgramsUpdated>(&event.data);
                match ev {
                    Ok(ev) => {
                        debug!("Parsed event: {:?}", ev);
                        let domain_ev = Updated {
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

async fn process_epg_retriever(mirakc_url: &str, nats_url: &str) {
    use domain::model::event::recording::{epg, programs};
    use mirakc::MirakcProgramsRetriever;

    debug!("EPGリトリーバーを開始します...");
    let nats_client = connect_nats(nats_url).await.unwrap();

    let epg_event_store = EventStore::<epg::Updated>::new(nats_client.clone())
        .await
        .unwrap();
    let programs_event_store = EventStore::<programs::Updated>::new(nats_client.clone())
        .await
        .unwrap();

    setup_kurec_streams(&nats_client).await.unwrap();

    let reader = epg_event_store
        .get_reader("epg-retriever".to_string())
        .await
        .unwrap();

    let programs_retriever = MirakcProgramsRetriever::new(mirakc_url);

    debug!("EPGイベント待機中...");

    loop {
        match reader.next().await {
            Ok(event) => {
                debug!("EPG更新イベントを受信: service_id={}", event.service_id);

                match programs_retriever.get_programs(event.service_id).await {
                    Ok(programs) => {
                        debug!(
                            "サービスID {} のプログラム {} 件を取得",
                            event.service_id,
                            programs.len()
                        );

                        let programs_updated = programs::Updated {
                            service_id: event.service_id,
                            programs,
                        };

                        match programs_event_store.publish_event(&programs_updated).await {
                            Ok(_) => debug!(
                                "プログラム更新イベントを発行しました: service_id={}",
                                event.service_id
                            ),
                            Err(e) => error!("プログラム更新イベントの発行に失敗: {:?}", e),
                        }
                    }
                    Err(e) => {
                        error!("プログラム情報の取得に失敗: {:?}", e);
                    }
                }
            }
            Err(e) => {
                error!("EPGイベントの受信に失敗: {:?}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::model::event::recording::{epg, programs};
    use domain::model::program::{Channel, Genre, Program, ProgramIdentifiers, ProgramTiming};
    use domain::ports::ProgramsRetriever;
    use std::sync::{Arc, Mutex};

    struct MockProgramsRetriever {
        service_id: i64,
        programs: Vec<Program>,
    }

    #[async_trait::async_trait]
    impl ProgramsRetriever for MockProgramsRetriever {
        async fn get_programs(&self, service_id: i64) -> Result<Vec<Program>, DomainError> {
            if service_id == self.service_id {
                Ok(self.programs.clone())
            } else {
                Err(DomainError::ServiceNotFound(service_id))
            }
        }
    }

    struct MockEventReader<T> {
        events: Vec<T>,
        current: usize,
    }

    impl<T: Clone> MockEventReader<T> {
        fn new(events: Vec<T>) -> Self {
            Self { events, current: 0 }
        }

        async fn next(&mut self) -> Result<T, String> {
            if self.current < self.events.len() {
                let event = self.events[self.current].clone();
                self.current += 1;
                Ok(event)
            } else {
                Err("イベントがありません".to_string())
            }
        }
    }

    struct MockEventStore<T> {
        published_events: Arc<Mutex<Vec<T>>>,
    }

    impl<T: Clone> MockEventStore<T> {
        fn new() -> Self {
            Self {
                published_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        async fn publish_event(&self, event: &T) -> Result<(), String> {
            self.published_events.lock().unwrap().push(event.clone());
            Ok(())
        }

        fn get_published_events(&self) -> Vec<T> {
            self.published_events.lock().unwrap().clone()
        }
    }

    #[tokio::test]
    async fn test_epg_retriever_logic() {
        let service_id = 1;
        let service_id_i32 = service_id as i32; // i64からi32への変換

        let test_program = Program::new(
            ProgramIdentifiers {
                id: 123456789,
                event_id: 1234,
                network_id: 5678,
                service_id: service_id_i32,
            },
            ProgramTiming {
                start_at: 1619856000000,
                duration: 1800000,
            },
            true,                                 // is_free
            Some("テスト番組".to_string()),       // name
            Some("テスト番組の説明".to_string()), // description
            vec![Genre { lv1: 7, lv2: 0 }],       // genres
            Channel {
                id: service_id,
                name: "テストチャンネル".to_string(),
            },
        );

        let mock_retriever = MockProgramsRetriever {
            service_id,
            programs: vec![test_program.clone()],
        };

        let epg_updated = epg::Updated { service_id };

        let programs = mock_retriever
            .get_programs(epg_updated.service_id)
            .await
            .unwrap();

        let programs_updated = programs::Updated {
            service_id: epg_updated.service_id,
            programs,
        };

        assert_eq!(programs_updated.service_id, service_id);
        assert_eq!(programs_updated.programs.len(), 1);

        let program = &programs_updated.programs[0];
        assert_eq!(program.id, 123456789);
        assert_eq!(program.service_id, service_id_i32);
        assert_eq!(program.name, Some("テスト番組".to_string()));
    }

    #[tokio::test]
    async fn test_process_epg_retriever_with_mocks() {
        let service_id = 1;
        let service_id_i32 = service_id as i32;

        let test_program = Program::new(
            ProgramIdentifiers {
                id: 123456789,
                event_id: 1234,
                network_id: 5678,
                service_id: service_id_i32,
            },
            ProgramTiming {
                start_at: 1619856000000,
                duration: 1800000,
            },
            true,
            Some("テスト番組".to_string()),
            Some("テスト番組の説明".to_string()),
            vec![Genre { lv1: 7, lv2: 0 }],
            Channel {
                id: service_id,
                name: "テストチャンネル".to_string(),
            },
        );

        let mock_retriever = MockProgramsRetriever {
            service_id,
            programs: vec![test_program.clone()],
        };

        let epg_updated = epg::Updated { service_id };

        let mut mock_reader = MockEventReader::new(vec![epg_updated.clone()]);

        let mock_event_store = MockEventStore::<programs::Updated>::new();

        // process_epg_retrieverの主要なロジックを再現
        let event = mock_reader.next().await.unwrap();

        let programs = mock_retriever.get_programs(event.service_id).await.unwrap();

        let programs_updated = programs::Updated {
            service_id: event.service_id,
            programs,
        };

        mock_event_store
            .publish_event(&programs_updated)
            .await
            .unwrap();

        let published_events = mock_event_store.get_published_events();
        assert_eq!(published_events.len(), 1);

        let published_event = &published_events[0];
        assert_eq!(published_event.service_id, service_id);
        assert_eq!(published_event.programs.len(), 1);

        let program = &published_event.programs[0];
        assert_eq!(program.id, 123456789);
        assert_eq!(program.service_id, service_id_i32);
        assert_eq!(program.name, Some("テスト番組".to_string()));
    }
}
