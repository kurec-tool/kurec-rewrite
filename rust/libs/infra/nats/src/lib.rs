//! NATS 接続と関連リソース管理のためのインフラクレート
//!
//! このクレートは NATS サーバーへの接続を確立し、
//! JetStream コンテキストや KV ストアへのアクセスを提供します。

use async_nats::{self, ConnectOptions, client::Client, connect_with_options, jetstream};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info};

#[derive(Error, Debug)]
pub enum NatsInfraError {
    #[error("NATS 接続に失敗しました: {0}")]
    ConnectionFailed(Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("JetStream コンテキストの取得に失敗しました: {0}")]
    JetStreamContextFailed(async_nats::Error),

    #[error("KV ストア '{bucket_name}' の作成/取得に失敗しました: {source}")]
    KvStoreFailed {
        bucket_name: String,
        source: async_nats::Error,
    },
}

/// NATS クライアントと関連コンテキストを保持するラッパー構造体。
#[derive(Clone, Debug)]
pub struct NatsClient {
    client: Client,
    js_context: jetstream::context::Context,
}

impl NatsClient {
    /// 新しい NatsClient インスタンスを作成します (内部利用)。
    fn new(client: Client) -> Self {
        let js_context = jetstream::new(client.clone());
        Self { client, js_context }
    }

    /// 接続済みの NATS クライアントを取得します。
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// 接続済みの JetStream コンテキストを取得します。
    pub fn jetstream_context(&self) -> &jetstream::context::Context {
        &self.js_context
    }
}

/// 指定された URL で NATS サーバーに接続し、`NatsClient` を返します。
///
/// 接続オプションには、再接続試行などのデフォルト設定が含まれます。
pub async fn connect(nats_url: &str) -> Result<NatsClient, NatsInfraError> {
    info!(url = %nats_url, "NATS サーバーへの接続を開始します...");

    // TODO: 設定ファイルから読み込むなど、より柔軟なオプション設定を検討
    let options = ConnectOptions::new()
        .retry_on_initial_connect()
        .connection_timeout(Duration::from_secs(10))
        .max_reconnects(None) // 無制限に再接続試行
        .reconnect_delay_callback(|attempts| {
            // 再接続試行回数に応じて遅延時間を調整 (例: 指数バックオフ)
            let delay = Duration::from_millis(100 * 2u64.pow(attempts.min(8) as u32)); // 最大約25秒
            debug!(attempts, delay = ?delay, "NATS 再接続試行...");
            delay
        });

    let client = connect_with_options(nats_url, options)
        .await
        .map_err(|e| NatsInfraError::ConnectionFailed(Box::new(e)))?;

    // 接続が確立されるまで待機し、接続状態を確認
    client
        .flush()
        .await
        .map_err(|e| NatsInfraError::ConnectionFailed(Box::new(e)))?;

    info!(url = %nats_url, "NATS サーバーへの接続が成功しました。");
    Ok(NatsClient::new(client))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use testcontainers::{
        ContainerAsync, GenericImage, ImageExt, core::WaitFor, runners::AsyncRunner,
    };
    use tracing::debug;

    // テスト終了時に自動的にコンテナを停止・削除するための構造体
    struct TestNatsContainer {
        _container: ContainerAsync<GenericImage>,
    }

    // Docker が利用可能かチェック
    async fn ensure_docker() {
        for _ in 0..20 {
            if std::process::Command::new("docker")
                .arg("info")
                .output()
                .is_ok()
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        panic!("Docker daemon not ready");
    }

    // テスト用の NATS サーバーを起動し、接続 URL とコンテナハンドラを返す
    async fn setup_nats() -> Result<(TestNatsContainer, String)> {
        ensure_docker().await;
        debug!("Starting NATS container for testing...");
        let container = GenericImage::new("nats", "latest")
            .with_exposed_port(4222u16.into())
            .with_wait_for(WaitFor::message_on_stderr("Server is ready"))
            .with_cmd(vec!["--js", "--debug"]) // JetStream を有効化
            .start()
            .await?;
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(4222u16).await?;
        let url = format!("nats://{}:{}", host, port); // スキームを nats:// に修正
        debug!(url = %url, "NATS container started.");

        // NATSサーバーが完全に起動するまで少し待機
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        Ok((
            TestNatsContainer {
                _container: container,
            },
            url,
        ))
    }

    // テスト用の NatsClient をセットアップするヘルパー関数
    async fn setup_test_client() -> Result<(TestNatsContainer, NatsClient)> {
        let (container_handler, url) = setup_nats().await?;
        let client = connect(&url).await?;
        Ok((container_handler, client))
    }

    #[tokio::test]
    async fn test_connect_success() -> Result<()> {
        let (_container_handler, client) = setup_test_client().await?;
        // flush() を呼び出して接続を確立させる
        client.client().flush().await?;
        // 接続状態を確認
        assert_eq!(
            client.client().connection_state(),
            async_nats::connection::State::Connected
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_get_jetstream_context() -> Result<()> {
        let (_container_handler, nats_client) = setup_test_client().await?;
        let js_ctx = nats_client.jetstream_context();
        // 簡単な操作を試す (例: アカウント情報取得 - query_account())
        let result = js_ctx.query_account().await;
        assert!(result.is_ok(), "JetStream 操作に失敗: {:?}", result.err());
        Ok(())
    }

    #[tokio::test]
    async fn test_connect_failure() -> Result<()> {
        // 存在しないポートに接続を試みる
        let non_existent_url = "nats://localhost:9999";

        // 再接続を無効にし、短いタイムアウトを設定したオプション
        let options = ConnectOptions::new()
            .max_reconnects(Some(0)) // 再接続を無効化
            .connection_timeout(Duration::from_secs(1)); // 短いタイムアウト

        let result = connect_with_options(non_existent_url, options).await;

        // 接続が失敗し、期待されるエラータイプが返されることを確認
        assert!(result.is_err());
        // async_nats::connect_with_options は async_nats::error::Error<async_nats::ConnectErrorKind> を返す
        match result.unwrap_err().kind() {
            // kind() メソッドで ConnectErrorKind を取得
            // 存在しないポートへの接続失敗は Io エラーになる可能性が高い
            async_nats::ConnectErrorKind::Io => {
                // 期待されるエラータイプ (I/O エラー)
                Ok(())
            }
            e => {
                // 予期しないエラータイプ
                panic!("予期しないエラーが発生しました: {:?}", e);
            }
        }
    }
}
