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
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("KVバケットから値の取得に失敗しました: {source}")]
    KvGet {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("KVバケットでの値の更新に失敗しました: {source}")]
    KvUpdate {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("KVバケットでの値の削除に失敗しました: {source}")]
    KvDelete {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
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

    #[error("メッセージの確認（ack）に失敗しました: {source}")]
    MessageAck {
        #[source]
        source: async_nats::Error,
    },
}
