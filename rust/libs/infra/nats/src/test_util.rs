//! Toxiproxy経由のNATS接続作成
//!
//! このモジュールでは、toxiproxyを使用してネットワーク障害をシミュレートし、
//! NATS クライアントをテストするためのユーティリティ関数を提供します。

use anyhow::Result;
use bollard::{Docker, network::CreateNetworkOptions};
use reqwest::Client as HttpClient;
use serde_json::json;
use std::{collections::HashMap, time::Duration};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{Host, WaitFor},
    runners::AsyncRunner,
};
use tokio::time;
use tracing::debug;
use tracing_subscriber::{EnvFilter, fmt};

/// テスト用のロギングを初期化
pub fn init_test_logging() {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
}

pub const PROXY_NAME: &str = "nats-proxy";

// テスト終了時に自動的にコンテナを停止・削除するための構造体
pub struct TestToxiproxyNatsContainer {
    pub api_url: String,
    pub nats_url: String,
    _nats_container: ContainerAsync<GenericImage>,
    _toxi_proxy_container: ContainerAsync<GenericImage>,
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
}

// テスト用の NATS サーバーを起動し、コンテナハンドラを返す
pub async fn setup_toxi_proxy_nats() -> Result<TestToxiproxyNatsContainer> {
    ensure_docker().await;
    debug!("Starting NATS container for testing...");

    let nats_container = GenericImage::new("nats", "latest")
        .with_exposed_port(4222u16.into())
        .with_wait_for(WaitFor::message_on_stderr("Server is ready"))
        .with_host("host.docker.internal", Host::HostGateway)
        .with_cmd(vec!["--js", "--debug"])
        .start()
        .await?;

    debug!("NATS container started on host network (localhost:4222)");

    debug!("Starting ToxiProxy container for testing...");
    let toxi_proxy_container = GenericImage::new("ghcr.io/shopify/toxiproxy", "latest")
        .with_exposed_port(8474u16.into())
        .with_exposed_port(4222u16.into())
        .with_wait_for(WaitFor::message_on_stdout("Starting Toxiproxy HTTP server"))
        .with_host("host.docker.internal", Host::HostGateway)
        .start()
        .await?;
    debug!("ToxiProxy container started.");

    // Toxiproxy サーバーが完全に起動するまで十分に待機
    // time::sleep(Duration::from_secs(15)).await;
    let toxi_host = toxi_proxy_container.get_host().await?;
    let toxi_api_port = toxi_proxy_container.get_host_port_ipv4(8474u16).await?;
    let toxi_nats_port = toxi_proxy_container.get_host_port_ipv4(4222u16).await?;
    let nats_host = "host.docker.internal";
    let nats_port = nats_container.get_host_port_ipv4(4222u16).await?;

    // Toxiproxy APIが応答するか確認
    let http_client = HttpClient::new();
    let api_url = format!("http://{}:{}", toxi_host, toxi_api_port);
    let nats_url = format!("nats://{}:{}", toxi_host, toxi_nats_port);
    debug!(api_url = %api_url, nats_host = %nats_host, toxi_host = %toxi_host, toxi_api_port = %toxi_api_port, toxi_nats_port = %toxi_nats_port, "Toxiproxy started.");

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

    let proxy_exists = check_proxy_exists(&http_client, &api_url, PROXY_NAME).await?;
    debug!(nats_host = %nats_host, nats_port = %nats_port, "プロキシを作成します");
    if !proxy_exists {
        // プロキシを作成
        create_proxy(
            &http_client,
            &api_url,
            PROXY_NAME,
            "0.0.0.0:4222",
            &format!("{}:{}", nats_host, nats_port),
        )
        .await?;
    }

    Ok(TestToxiproxyNatsContainer {
        _nats_container: nats_container,
        _toxi_proxy_container: toxi_proxy_container,
        api_url,
        nats_url,
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
pub async fn disable_proxy(
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
pub async fn enable_proxy(
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
