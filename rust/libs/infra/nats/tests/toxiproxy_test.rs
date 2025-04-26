//! NATS 再接続機能のテスト
//!
//! このモジュールでは、toxiproxyを使用してネットワーク障害をシミュレートし、
//! NATS クライアントの再接続機能をテストします。

use anyhow::Result;
use nats::connect;
use reqwest::Client as HttpClient;
use serde_json::json;
use std::time::Duration;
use testcontainers::{ContainerAsync, GenericImage, ImageExt, core::WaitFor, runners::AsyncRunner};
use tokio::{process::Command, time};
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt};

fn init_test_logging() {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
}

// テスト終了時に自動的にコンテナを停止・削除するための構造体
struct TestToxiproxyContainer {
    host: String,
    port: u16,
}

struct TestNatsContainer {
    _container: ContainerAsync<GenericImage>,
    host: String,
    port: u16,
}

// Docker が利用可能かチェック
async fn ensure_docker() {
    // Docker デーモンが準備できるまで待機
    for _ in 0..20 {
        if std::process::Command::new("docker")
            .arg("info")
            .output()
            .is_ok()
        {
            break;
        }
        time::sleep(Duration::from_secs(1)).await;
    }

    let network_output = std::process::Command::new("docker")
        .args(["network", "create", "toxiproxy-test-network"])
        .output();

    match network_output {
        Ok(output) => {
            if !output.status.success()
                && !String::from_utf8_lossy(&output.stderr).contains("already exists")
            {
                debug!(
                    "ネットワーク作成エラー: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                panic!("Docker ネットワークの作成に失敗しました");
            }
            debug!("Docker ネットワーク 'toxiproxy-test-network' の準備完了");
        }
        Err(e) => {
            panic!("Docker ネットワークの作成に失敗しました: {}", e);
        }
    }
}

// テスト用の Toxiproxy サーバーを起動し、コンテナハンドラを返す
async fn setup_toxiproxy() -> Result<TestToxiproxyContainer> {
    ensure_docker().await;
    debug!("Starting Toxiproxy container for testing...");

    // Toxiproxy コンテナを起動
    let toxiproxy_container_id = Command::new("docker")
        .args([
            "run",
            "-d",
            "--network",
            "host", // ホストネットワークを使用
            "--name",
            "toxiproxy",
            "-e",
            "LOG_LEVEL=debug",
            "-e",
            "EXTRA_HOSTS=host.docker.internal:host-gateway",
            "shopify/toxiproxy:latest",
        ])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Toxiproxyコンテナの起動に失敗: {}", e))?;

    let container_id = String::from_utf8_lossy(&toxiproxy_container_id.stdout)
        .trim()
        .to_string();
    debug!("Toxiproxy container started with ID: {}", container_id);

    time::sleep(Duration::from_secs(5)).await;

    let container = TestToxiproxyContainer {
        host: "localhost".to_string(),
        port: 8474,
    };

    let host = container.host.clone();
    let port = container.port;

    debug!(host = %host, port = %port, "Toxiproxy container started.");

    // Toxiproxy サーバーが完全に起動するまで十分に待機
    time::sleep(Duration::from_secs(15)).await;

    // Toxiproxy APIが応答するか確認
    let http_client = HttpClient::new();
    let api_url = format!("http://{}:{}", host, port);

    for _ in 0..5 {
        match http_client.get(&api_url).send().await {
            Ok(_) => {
                debug!("Toxiproxy API is responding");
                break;
            }
            Err(e) => {
                debug!(error = %e, "Toxiproxy API not responding yet, retrying...");
                time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Ok(TestToxiproxyContainer {
        host: host.to_string(),
        port,
    })
}

// テスト用の NATS サーバーを起動し、コンテナハンドラを返す
async fn setup_nats() -> Result<TestNatsContainer> {
    ensure_docker().await;
    debug!("Starting NATS container for testing...");

    let container = GenericImage::new("nats", "latest")
        .with_exposed_port(4222u16.into())
        .with_wait_for(WaitFor::message_on_stderr("Server is ready"))
        .with_cmd(vec!["--js", "--debug"]) // JetStream を有効化
        .with_network("host") // ホストネットワークを使用
        .with_name("nats") // コンテナ名を明示的に指定
        .start()
        .await?;

    debug!("NATS container started on host network (localhost:4222)");

    // NATS サーバーが完全に起動するまで少し待機
    time::sleep(Duration::from_secs(2)).await;

    Ok(TestNatsContainer {
        _container: container,
        host: "localhost".to_string(),
        port: 4222,
    })
}

// Toxiproxy API を使用してプロキシを作成する
// Toxiproxy API を使用してプロキシの状態を確認する
async fn check_proxy_exists(
    http_client: &HttpClient,
    toxiproxy_url: &str,
    proxy_name: &str,
) -> Result<bool> {
    let url = format!("{}/proxies/{}", toxiproxy_url, proxy_name);

    let response = http_client.get(&url).send().await?;

    if response.status().is_success() {
        debug!(proxy_name = %proxy_name, "Toxiproxy プロキシが存在します");
        Ok(true)
    } else if response.status().as_u16() == 404 {
        debug!(proxy_name = %proxy_name, "Toxiproxy プロキシが存在しません");
        Ok(false)
    } else {
        let error_text = response.text().await?;
        anyhow::bail!("プロキシの状態確認に失敗しました: {}", error_text);
    }
}

async fn create_proxy(
    http_client: &HttpClient,
    toxiproxy_url: &str,
    proxy_name: &str,
    listen_addr: &str,
    upstream_addr: &str,
) -> Result<()> {
    let url = format!("{}/proxies", toxiproxy_url);

    let proxy_config = json!({
        "name": proxy_name,
        "listen": listen_addr,
        "upstream": upstream_addr,
        "enabled": true
    });

    let response = http_client.post(&url).json(&proxy_config).send().await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("プロキシの作成に失敗しました: {}", error_text);
    }

    debug!(
        proxy_name = %proxy_name,
        listen = %listen_addr,
        upstream = %upstream_addr,
        "Toxiproxy プロキシを作成しました"
    );

    Ok(())
}

// Toxiproxy API を使用してプロキシを無効化する
async fn disable_proxy(
    http_client: &HttpClient,
    toxiproxy_url: &str,
    proxy_name: &str,
) -> Result<()> {
    let url = format!("{}/proxies/{}", toxiproxy_url, proxy_name);

    let proxy_config = json!({
        "enabled": false
    });

    let response = http_client.post(&url).json(&proxy_config).send().await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("プロキシの無効化に失敗しました: {}", error_text);
    }

    debug!(proxy_name = %proxy_name, "Toxiproxy プロキシを無効化しました");

    Ok(())
}

// Toxiproxy API を使用してプロキシを有効化する
async fn enable_proxy(
    http_client: &HttpClient,
    toxiproxy_url: &str,
    proxy_name: &str,
) -> Result<()> {
    let url = format!("{}/proxies/{}", toxiproxy_url, proxy_name);

    let proxy_config = json!({
        "enabled": true
    });

    let response = http_client.post(&url).json(&proxy_config).send().await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("プロキシの有効化に失敗しました: {}", error_text);
    }

    debug!(proxy_name = %proxy_name, "Toxiproxy プロキシを有効化しました");

    Ok(())
}

// 基本的な接続テスト
#[tokio::test]
async fn test_toxiproxy_basic_connection() -> Result<()> {
    init_test_logging();

    // Toxiproxy コンテナを起動
    let toxiproxy_container = setup_toxiproxy().await?;

    // NATS コンテナを起動
    let _nats_container = setup_nats().await?;

    time::sleep(Duration::from_secs(5)).await;

    // HTTP クライアントを作成
    let http_client = HttpClient::new();

    // Toxiproxy API の URL
    let toxiproxy_url = format!(
        "http://{}:{}",
        toxiproxy_container.host, toxiproxy_container.port
    );

    // プロキシ名
    let proxy_name = "nats-proxy";

    // プロキシのリッスンアドレス (Toxiproxy コンテナ内)
    let listen_addr = "0.0.0.0:4223";

    // アップストリームアドレス (NATS コンテナ)
    // Docker ネットワーク内ではコンテナ名で解決できる
    let upstream_addr = "localhost:4222".to_string();
    debug!("NATS upstream address: {}", upstream_addr);

    let proxy_exists = check_proxy_exists(&http_client, &toxiproxy_url, proxy_name).await?;

    if !proxy_exists {
        // プロキシを作成
        create_proxy(
            &http_client,
            &toxiproxy_url,
            proxy_name,
            listen_addr,
            &upstream_addr,
        )
        .await?;
    }

    // プロキシ経由の NATS URL
    let proxy_port = 4223;
    let nats_url = format!("nats://127.0.0.1:{}", proxy_port);

    debug!(url = %nats_url, "Toxiproxy 経由で NATS に接続します");

    // プロキシ経由で NATS に接続
    let client = connect(&nats_url).await?;

    // 接続状態を確認
    client.client().flush().await?;
    assert_eq!(
        client.client().connection_state(),
        async_nats::connection::State::Connected
    );

    debug!("Toxiproxy 経由での基本的な接続テストが成功しました");

    Ok(())
}

// 再接続テスト
#[tokio::test]
async fn test_nats_reconnection() -> Result<()> {
    init_test_logging();

    // Toxiproxy コンテナを起動
    let toxiproxy_container = setup_toxiproxy().await?;

    // NATS コンテナを起動
    let _nats_container = setup_nats().await?;

    time::sleep(Duration::from_secs(5)).await;

    // HTTP クライアントを作成
    let http_client = HttpClient::new();

    // Toxiproxy API の URL
    let toxiproxy_url = format!(
        "http://{}:{}",
        toxiproxy_container.host, toxiproxy_container.port
    );

    // プロキシ名
    let proxy_name = "nats-proxy";

    // プロキシのリッスンアドレス (Toxiproxy コンテナ内)
    let listen_addr = "0.0.0.0:4223";

    // アップストリームアドレス (NATS コンテナ)
    // Docker ネットワーク内ではコンテナ名で解決できる
    let upstream_addr = "localhost:4222".to_string();
    debug!("NATS upstream address: {}", upstream_addr);

    let proxy_exists = check_proxy_exists(&http_client, &toxiproxy_url, proxy_name).await?;

    if !proxy_exists {
        // プロキシを作成
        create_proxy(
            &http_client,
            &toxiproxy_url,
            proxy_name,
            listen_addr,
            &upstream_addr,
        )
        .await?;
    }

    // プロキシ経由の NATS URL
    let proxy_port = 4223;
    let nats_url = format!("nats://127.0.0.1:{}", proxy_port);

    debug!(url = %nats_url, "Toxiproxy 経由で NATS に接続します");

    // まず通常接続を確認
    let client = connect(&nats_url).await?;
    client.client().flush().await?;
    assert_eq!(
        client.client().connection_state(),
        async_nats::connection::State::Connected
    );

    // プロキシを無効化
    disable_proxy(&http_client, &toxiproxy_url, proxy_name).await?;

    // tokio::select を使って並列処理を実装
    // 1. connect() を呼び出す
    // 2. 1秒待ってからプロキシを元に戻す
    let reconnection_result = tokio::select! {
        connect_result = connect(&nats_url) => {
            debug!("connect() が完了しました");
            connect_result
        }
        _ = async {
            // 1秒待機
            time::sleep(Duration::from_secs(1)).await;
            // プロキシを元に戻す
            enable_proxy(&http_client, &toxiproxy_url, proxy_name).await?;
            debug!("プロキシを再有効化しました");
            Ok::<_, anyhow::Error>(())
        } => {
            // 再接続を待機
            time::sleep(Duration::from_secs(3)).await;
            debug!("再接続を待機しています...");
            connect(&nats_url).await
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

// 単純なテストを追加して、テストフレームワークが正常に動作することを確認
#[tokio::test]
async fn test_dummy() {
    // 単純なテストケース
    assert_eq!(1 + 1, 2);
}
