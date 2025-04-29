use std::vec;

use clap::{Parser, Subcommand};
use domain::model::event::recording::epg::Updated;
use domain::model::program::ProgramsData;
use domain::ports::ProgramsRetriever;
use domain::repository::KvRepository;
use futures::StreamExt as _;
use mirakc::get_mirakc_event_stream;
use nats::{
    nats::connect_nats,
    repositories::ProgramsDataRepository,
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
    OgpUrlExtractor {
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
        Commands::OgpUrlExtractor { nats_url } => {
            process_ogp_url_extractor(nats_url).await;
        }
    }
}

async fn setup_kurec_streams(
    nats_client: &nats::nats::NatsClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream_configs = vec![
        StreamConfig {
            name: "kurec".to_string(),
            subjects: vec![
                "recording.>".to_string(), // すべてのrecordingイベントをカバー
            ],
            ..Default::default()
        },
        StreamConfig {
            name: "kurec-ogp".to_string(),
            subjects: vec!["ogp.>".to_string()],
            ..Default::default()
        },
    ];

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
                            mirakc_url: mirakc_url.to_string(),
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
    use nats::kvs::NatsKvRepositoryTrait;

    debug!("EPGリトリーバーを開始します...");
    let nats_client = connect_nats(nats_url).await.unwrap();

    let epg_event_store = EventStore::<epg::Updated>::new(nats_client.clone())
        .await
        .unwrap();
    let programs_event_store = EventStore::<programs::Updated>::new(nats_client.clone())
        .await
        .unwrap();

    let programs_kvs_repo = ProgramsDataRepository::new(nats_client.clone())
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
            Ok((event, mut ack_handle)) => {
                let service_id = event.service_id;
                debug!("EPG更新イベントを受信: service_id={}", service_id);

                match programs_retriever.get_programs(service_id).await {
                    Ok(programs) => {
                        debug!(
                            "サービスID {} のプログラム {} 件を取得",
                            service_id,
                            programs.len()
                        );

                        let key = service_id.to_string();
                        let programs_data = ProgramsData(programs);
                        if let Err(e) = programs_kvs_repo.put(key.clone(), &programs_data).await {
                            error!("KVSへのプログラムデータ保存に失敗: {}", e);
                            continue;
                        }

                        let programs_updated = programs::Updated {
                            service_id,
                            mirakc_url: mirakc_url.to_string(),
                        };

                        match programs_event_store.publish_event(&programs_updated).await {
                            Ok(_) => {
                                debug!(
                                    "プログラム更新イベントを発行しました: service_id={}",
                                    service_id
                                );
                                if let Err(e) = ack_handle.ack().await {
                                    error!("メッセージの確認（ack）に失敗: {:?}", e);
                                }
                            }
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

async fn process_ogp_url_extractor(nats_url: &str) {
    use domain::model::event::{ogp, recording::programs};
    use domain::model::url_extractor::UrlExtractor;
    use nats::kvs::NatsKvRepositoryTrait;

    debug!("OGP URL抽出ワーカーを開始します...");
    let nats_client = connect_nats(nats_url).await.unwrap();

    let programs_event_store = EventStore::<programs::Updated>::new(nats_client.clone())
        .await
        .unwrap();
    let ogp_event_store = EventStore::<ogp::url::ExtractRequest>::new(nats_client.clone())
        .await
        .unwrap();

    let programs_kvs_repo = ProgramsDataRepository::new(nats_client.clone())
        .await
        .unwrap();

    setup_kurec_streams(&nats_client).await.unwrap();

    debug!("プログラム更新イベント待機中...");

    let reader = programs_event_store
        .get_reader("ogp_url_extractor".to_string())
        .await
        .unwrap();

    loop {
        match reader.next().await {
            Ok((event, mut ack_handle)) => {
                let service_id = event.service_id;
                debug!("プログラム更新イベントを受信: service_id={}", service_id);

                let key = service_id.to_string();
                match programs_kvs_repo.get(key).await {
                    Ok(Some(versioned)) => {
                        let programs_data = versioned.value;
                        let extractor = UrlExtractor::default();

                        for program in &programs_data.0 {
                            if let Some(extended) = &program.extended {
                                for value in extended.values() {
                                    let urls = extractor.extract_urls(value);

                                    for url in urls {
                                        debug!("Found URL from program {}: {}", program.id, url);
                                        let ogp_event = ogp::url::ExtractRequest { url };
                                        if let Err(e) =
                                            ogp_event_store.publish_event(&ogp_event).await
                                        {
                                            error!("OGPリクエストイベントの発行に失敗: {:?}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        debug!(
                            "プログラムデータが見つかりません: service_id={}",
                            service_id
                        );
                    }
                    Err(e) => {
                        error!("KVSからのプログラムデータ取得に失敗: {:?}", e);
                    }
                }

                if let Err(e) = ack_handle.ack().await {
                    error!("イベントの確認に失敗: {:?}", e);
                }
            }
            Err(e) => {
                error!("イベントの取得に失敗: {:?}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::{Arc, Mutex};

    use crate::ProgramsData;
    use bytes::Bytes;
    use domain::error::DomainError;
    use domain::model::event::{
        ogp,
        recording::{epg, programs},
    };
    use domain::model::program::{Channel, Genre, Program, ProgramIdentifiers, ProgramTiming};
    use domain::model::url_extractor::UrlExtractor;
    use domain::ports::ProgramsRetriever;
    use domain::repository::{KvRepository, Versioned};

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

    struct MockKvRepository<V>
    where
        V: Into<Bytes> + From<Bytes> + Clone + Send + Sync,
    {
        data: Arc<Mutex<HashMap<String, (u64, V)>>>,
    }

    impl<V> MockKvRepository<V>
    where
        V: Into<Bytes> + From<Bytes> + Clone + Send + Sync,
    {
        fn new() -> Self {
            Self {
                data: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl<V> KvRepository<String, V> for MockKvRepository<V>
    where
        V: Into<Bytes> + From<Bytes> + Clone + Send + Sync,
    {
        async fn put(&self, key: String, value: &V) -> Result<(), DomainError> {
            let mut data = self.data.lock().unwrap();
            let revision = data.get::<str>(key.as_ref()).map_or(1, |(rev, _)| rev + 1);
            data.insert(key, (revision, value.clone()));
            Ok(())
        }

        async fn get(&self, key: String) -> Result<Option<Versioned<V>>, DomainError> {
            let data = self.data.lock().unwrap();
            if let Some((revision, value)) = data.get::<str>(key.as_ref()) {
                Ok(Some(Versioned {
                    revision: *revision,
                    value: value.clone(),
                }))
            } else {
                Ok(None)
            }
        }

        async fn update(&self, key: String, value: &V, revision: u64) -> Result<(), DomainError> {
            let mut data = self.data.lock().unwrap();
            if let Some((current_revision, _)) = data.get::<str>(key.as_ref()) {
                if *current_revision != revision {
                    return Err(DomainError::ProgramsStoreError(
                        "リビジョンが一致しません".to_string(),
                    ));
                }
            } else {
                return Err(DomainError::ProgramsStoreError(
                    "存在しないキーを更新しようとしました".to_string(),
                ));
            }

            data.insert(key, (revision + 1, value.clone()));
            Ok(())
        }

        async fn delete(&self, key: String) -> Result<(), DomainError> {
            let mut data = self.data.lock().unwrap();
            data.remove::<str>(key.as_ref());
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_epg_retriever_logic() {
        let service_id = 1;
        let service_id_i32 = service_id as i32; // i64からi32への変換
        let mirakc_url = "http://example.com";

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

        let epg_updated = epg::Updated {
            service_id,
            mirakc_url: mirakc_url.to_string(),
        };

        let programs = mock_retriever
            .get_programs(epg_updated.service_id)
            .await
            .unwrap();

        let mirakc_url = "http://example.com";
        let programs_updated = programs::Updated {
            service_id: epg_updated.service_id,
            mirakc_url: mirakc_url.to_string(),
        };

        assert_eq!(programs_updated.service_id, service_id);
        assert_eq!(programs_updated.mirakc_url, mirakc_url);

        assert_eq!(programs.len(), 1);
        let program = &programs[0];
        assert_eq!(program.id, 123456789);
        assert_eq!(program.service_id, service_id_i32);
        assert_eq!(program.name, Some("テスト番組".to_string()));
    }

    #[tokio::test]
    async fn test_process_epg_retriever_with_mocks() {
        let service_id = 1;
        let service_id_i32 = service_id as i32;
        let mirakc_url = "http://example.com";

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

        let epg_updated = epg::Updated {
            service_id,
            mirakc_url: mirakc_url.to_string(),
        };

        let mut mock_reader = MockEventReader::new(vec![epg_updated.clone()]);

        let mock_event_store = MockEventStore::<programs::Updated>::new();
        let mock_kvs_repo = MockKvRepository::<ProgramsData>::new();

        // process_epg_retrieverの主要なロジックを再現
        let event = mock_reader.next().await.unwrap();

        let programs = mock_retriever.get_programs(event.service_id).await.unwrap();

        let key = event.service_id.to_string();
        let programs_data = ProgramsData(programs);
        mock_kvs_repo
            .put(key.clone(), &programs_data)
            .await
            .unwrap();

        let programs_updated = programs::Updated {
            service_id: event.service_id,
            mirakc_url: mirakc_url.to_string(),
        };

        mock_event_store
            .publish_event(&programs_updated)
            .await
            .unwrap();

        let published_events = mock_event_store.get_published_events();
        assert_eq!(published_events.len(), 1);

        let published_event = &published_events[0];
        assert_eq!(published_event.service_id, service_id);
        assert_eq!(published_event.mirakc_url, mirakc_url);

        let stored_programs_data = mock_kvs_repo.get(key).await.unwrap().unwrap().value;
        let stored_programs = stored_programs_data.0;
        assert_eq!(stored_programs.len(), 1);
        assert_eq!(stored_programs[0].id, 123456789);
        assert_eq!(stored_programs[0].service_id, service_id_i32);
        assert_eq!(stored_programs[0].name, Some("テスト番組".to_string()));
    }
    #[tokio::test]
    async fn test_ogp_url_extractor() {
        let service_id = 1;

        let mut program = Program::new(
            ProgramIdentifiers {
                id: 1,
                event_id: 1234,
                network_id: 5678,
                service_id: 1,
            },
            ProgramTiming {
                start_at: 1619856000000,
                duration: 1800000,
            },
            true,
            Some("テスト番組".to_string()),
            Some("テスト説明".to_string()),
            vec![Genre { lv1: 7, lv2: 0 }],
            Channel {
                id: 1,
                name: "テストチャンネル".to_string(),
            },
        );

        let mut extended = BTreeMap::new();
        extended.insert(
            "description".to_string(),
            "これはテスト説明です。https://example.com に詳細があります。".to_string(),
        );
        extended.insert(
            "info".to_string(),
            "詳細は http://example.com/long/path/to/url/index.html?param=value#section を参照してください。".to_string(),
        );
        program.extended = Some(extended);

        let programs_data = ProgramsData(vec![program]);
        let versioned = Versioned {
            revision: 1,
            value: programs_data,
        };

        let kvs = MockKvRepository::<ProgramsData>::new();
        kvs.data
            .lock()
            .unwrap()
            .insert(service_id.to_string(), (1, versioned.value.clone()));

        let event = programs::Updated {
            service_id,
            mirakc_url: "http://mirakc:40772".to_string(),
        };
        let mut reader = MockEventReader::new(vec![event]);

        let store = MockEventStore::<ogp::url::ExtractRequest>::new();

        // process_ogp_url_extractorの主要なロジックを再現
        let event = reader.next().await.unwrap();
        let key = event.service_id.to_string();

        if let Some(versioned) = kvs.get(key).await.unwrap() {
            let programs_data = versioned.value;
            let extractor = UrlExtractor::default();

            for program in &programs_data.0 {
                if let Some(extended) = &program.extended {
                    for value in extended.values() {
                        let urls = extractor.extract_urls(value);

                        for url in urls {
                            let ogp_event = ogp::url::ExtractRequest { url };
                            store.publish_event(&ogp_event).await.unwrap();
                        }
                    }
                }
            }
        }

        let published_events = store.get_published_events();
        assert_eq!(published_events.len(), 2);

        let urls: Vec<String> = published_events.iter().map(|e| e.url.clone()).collect();

        assert!(urls.contains(&"https://example.com".to_string()));
        assert!(urls.contains(
            &"http://example.com/long/path/to/url/index.html?param=value#section".to_string()
        ));
    }
}
