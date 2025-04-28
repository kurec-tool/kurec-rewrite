//! NATS 接続と関連リソース管理のためのインフラクレート
//!
//! このクレートは NATS サーバーへの接続を確立し、
//! JetStream コンテキストや KV ストアへのアクセスを提供します。

use async_nats::{self, ConnectOptions, client::Client, connect_with_options, jetstream};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::NatsInfraError;

/// NATS クライアントと関連コンテキストを保持するラッパー構造体。
#[derive(Clone, Debug)]
pub struct NatsClient {
    _client: Client,
    js_context: jetstream::context::Context,
}

impl NatsClient {
    /// 新しい NatsClient インスタンスを作成します (内部利用)。
    fn new(client: Client) -> Self {
        let js_context = jetstream::new(client.clone());
        Self {
            _client: client,
            js_context,
        }
    }

    /// 接続済みの NATS クライアントを取得します。
    #[cfg(test)]
    pub(crate) fn client(&self) -> &Client {
        &self._client
    }

    /// 接続済みの JetStream コンテキストを取得します。
    pub(crate) fn jetstream_context(&self) -> &jetstream::context::Context {
        &self.js_context
    }
}

/// 指定された URL で NATS サーバーに接続し、`NatsClient` を返します。
///
/// 接続オプションには、再接続試行などのデフォルト設定が含まれます。
pub async fn connect_nats(nats_url: &str) -> Result<NatsClient, NatsInfraError> {
    info!(url = %nats_url, "NATS サーバーへの接続を開始します...");

    // TODO: 設定ファイルから読み込むなど、より柔軟なオプション設定を検討
    let options = ConnectOptions::new()
        .retry_on_initial_connect()
        .connection_timeout(Duration::from_secs(10))
        .max_reconnects(None) // 無制限に再接続試行
        .reconnect_delay_callback(|attempts| {
            // 再接続試行回数に応じて遅延時間を調整 (例: 指数バックオフ)
            let delay = Duration::from_millis(100 * 2u64.pow(attempts.min(8) as u32)); // 最大約25秒
            if attempts > 3 {
                warn!(attempts, delay = ?delay, "NATS 再接続試行...");
            } else {
                debug!(attempts, delay = ?delay, "NATS 再接続試行...");
            }
            delay
        });

    let client = connect_with_options(nats_url, options)
        .await
        .map_err(|e| NatsInfraError::Connection(Box::new(e)))?;

    // 接続が確立されるまで待機し、接続状態を確認
    client
        .flush()
        .await
        .map_err(|e| NatsInfraError::Connection(Box::new(e)))?;

    info!(url = %nats_url, "NATS サーバーへの接続が成功しました。");
    Ok(NatsClient::new(client))
}

#[cfg(test)]
mod tests {
    use crate::test_util::{
        PROXY_NAME, disable_proxy, enable_proxy, init_test_logging, setup_toxi_proxy_nats,
    };

    use super::*;
    // use crate::test_util::{
    //     PROXY_NAME, disable_proxy, enable_proxy, init_test_logging, setup_toxi_proxy_nats,
    // };
    use anyhow::Result;
    use tokio::time;
    use tracing::debug;

    #[tokio::test]
    async fn test_connect_success() -> Result<()> {
        let proxy = setup_toxi_proxy_nats().await?;
        let client = connect_nats(&proxy.nats_url).await?;
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
        let proxy = setup_toxi_proxy_nats().await?;
        let client = connect_nats(&proxy.nats_url).await?;
        let js_ctx = client.jetstream_context();
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

    // 再接続テスト
    #[tokio::test]
    async fn test_nats_reconnection() -> Result<()> {
        init_test_logging();

        // NATS コンテナを起動
        let toxi_proxy_nats_container = setup_toxi_proxy_nats().await?;

        time::sleep(Duration::from_secs(5)).await;

        // HTTP クライアントを作成
        let http_client = reqwest::Client::new();

        // Toxiproxy API の URL
        let toxiproxy_url = &toxi_proxy_nats_container.api_url;

        // プロキシ名
        let proxy_name = PROXY_NAME;

        // アップストリームアドレス (NATS コンテナ)
        // Docker ネットワーク内ではコンテナ名で解決できる
        let upstream_addr = "localhost:4222".to_string();
        debug!("NATS upstream address: {}", upstream_addr);

        // プロキシ経由の NATS URL
        let nats_url = toxi_proxy_nats_container.nats_url.clone();

        debug!(url = %nats_url, "Toxiproxy 経由で NATS に接続します");

        // まず通常接続を確認
        let client = connect_nats(&nats_url).await?;
        client.client().flush().await?;
        assert_eq!(
            client.client().connection_state(),
            async_nats::connection::State::Connected
        );

        // プロキシを無効化
        disable_proxy(&http_client, toxiproxy_url, proxy_name).await?;

        // tokio::select を使って並列処理を実装
        // 1. connect() を呼び出す
        // 2. 1秒待ってからプロキシを元に戻す
        let reconnection_result = tokio::select! {
            connect_result = connect_nats(&nats_url) => {
                debug!("connect() が完了しました");
                connect_result
            }
            _ = async {
                // 1秒待機
                time::sleep(Duration::from_secs(1)).await;
                // プロキシを元に戻す
                enable_proxy(&http_client, toxiproxy_url, proxy_name).await?;
                debug!("プロキシを再有効化しました");
                Ok::<_, anyhow::Error>(())
            } => {
                // 再接続を待機
                time::sleep(Duration::from_secs(3)).await;
                debug!("再接続を待機しています...");
                connect_nats(&nats_url).await
            }
        };

        // 再接続が成功したことを確認
        let reconnected_client = reconnection_result?;
        reconnected_client.client().flush().await?;
        assert_eq!(
            reconnected_client.client().connection_state(),
            async_nats::connection::State::Connected
        );

        debug!("NATS 再接続テストが成功しました");
        Ok(())
    }
}
