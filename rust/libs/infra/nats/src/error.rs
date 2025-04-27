use domain::types::Event;

/// インフラ層でのエラー
#[derive(thiserror::Error, Debug)]
pub enum NatsInfraError {
    #[error("NATS 接続に失敗しました: {0}")]
    Connection(Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("JetStream コンテキストの取得に失敗しました: {0}")]
    JetStreamContext(async_nats::Error),

    #[error("KV ストア '{bucket_name}' の作成/取得に失敗しました: {source}")]
    KvStore {
        bucket_name: String,
        source: async_nats::Error,
    },

    #[error("JetStream ストリームの作成に失敗しました: {stream_name}: {source}")]
    StreamCreation {
        stream_name: String,
        source: async_nats::Error,
    },

    #[error("JetStream ストリーム '{stream_name}' の取得に失敗しました: {source}")]
    StreamRetrieval {
        stream_name: String,
        source: async_nats::Error,
    },

    #[error("JetStream ストリームへイベント{subject}の発行に失敗しました: {source}")]
    EventPublish {
        subject: String,
        source: async_nats::Error,
    },

    #[error("JSON シリアライズ/デシリアライズエラー: {source}")]
    JsonSerialize {
        subject: String,
        source: serde_json::Error,
    },

    #[error("JSON デシリアライズエラー: {source}")]
    JsonDeserialize {
        subject: String,
        message: Vec<u8>,
        source: serde_json::Error,
    },
}
