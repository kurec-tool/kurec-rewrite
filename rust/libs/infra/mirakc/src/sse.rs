//! mirakc SSEイベントソースの実装

use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use chrono::Utc;
use eventsource_client::{Client, SSE};
use futures::{StreamExt, stream::BoxStream};
use thiserror::Error;
use tracing::{debug, error};

/// mirakc SSE接続エラー
#[derive(Error, Debug)]
pub enum MirakcSseConnectionError {
    #[error("SSE ストリームの接続中にエラーが発生しました: {0}")]
    SseStreamError(#[from] eventsource_client::Error),
}

/// mirakc SSEイベントエラー
#[derive(Error, Debug)]
pub enum MirakcSseStreamError {
    #[error("SSE ストリームの受信中にエラーが発生しました: {0}")]
    EventSourceError(#[from] eventsource_client::Error),
}

#[derive(Clone, Debug)]
pub struct MirakcEventInput {
    mirakc_url: String,
    event_type: String,
    data: String,
    received_at: chrono::DateTime<Utc>,
}

/// mirakc SSEイベントストリームを取得する
/// retry_max = 0で無限回再試行
pub async fn get_mirakc_event_stream(
    mirakc_url: &str,
    retry_max: u32,
) -> Result<BoxStream<'static, MirakcEventInput>, MirakcSseConnectionError> {
    let url = format!("{}/events", mirakc_url);
    let reconnect_options = eventsource_client::ReconnectOptions::reconnect(false)
        .retry_initial(false)
        .delay(Duration::from_secs(1))
        .backoff_factor(2)
        .delay_max(Duration::from_secs(10))
        .build();
    let client = eventsource_client::ClientBuilder::for_url(&url)
        .map_err(MirakcSseConnectionError::SseStreamError)?
        .connect_timeout(Duration::from_secs(1))
        .read_timeout(Duration::from_secs(1))
        .write_timeout(Duration::from_secs(1))
        .reconnect(reconnect_options)
        .build();
    debug!("SSEクライアントを構築完了: {}", url);
    let mirakc_url_cloned = mirakc_url.to_string();
    let retry_count = Arc::new(AtomicU32::new(0));
    let retry_count_clone = retry_count.clone();
    let stream = client
        .stream()
        .take_while(move |_| {
            let retry_count = retry_count_clone.clone();
            async move {
                let count = retry_count.load(Ordering::SeqCst);
                retry_max == 0 || count < retry_max
            }
        })
        .filter_map(move |event| {
            let mirakc_url_cloned = mirakc_url_cloned.clone();
            let retry_count = retry_count.clone();
            async move {
                match event {
                    Ok(SSE::Connected(ev)) => {
                        debug!("SSE接続成功: {:?}", ev);
                        None
                    }
                    Ok(SSE::Comment(ev)) => {
                        debug!("SSEコメント: {:?}", ev);
                        None
                    }
                    Ok(SSE::Event(ev)) => {
                        let event = MirakcEventInput {
                            mirakc_url: mirakc_url_cloned,
                            event_type: ev.event_type,
                            data: ev.data,
                            received_at: Utc::now(),
                        };
                        Some(event)
                    }
                    Err(e) => {
                        let prev_count = retry_count.fetch_add(1, Ordering::SeqCst);
                        error!("SSEエラー[{}]: {:?}", prev_count, e);
                        let dur = Duration::from_secs(1);
                        tokio::time::sleep(dur).await;
                        // このエラーは無視してストリームを続行
                        // 再試行回数が規定回数以上で、次のtake_whileで終了する
                        None
                    }
                }
            }
        })
        .boxed();
    Ok(stream)
}
#[cfg(test)]
mod tests {
    use test_util::init_test_logging;

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_get_mirakc_event_stream_success() {
        let url = "http://tuner:40772";
        let mut stream = get_mirakc_event_stream(url, 5).await.unwrap();

        let event = stream.next().await.unwrap();
        assert_eq!(event.mirakc_url, url);
        assert_eq!(event.event_type, "epg.programs-updated");
        assert!(!event.data.is_empty());
        assert_eq!(event.received_at, Utc::now()); // eqなわけない
        dbg!(event);
    }

    // 実チューナーに接続しにいくのでignore
    #[tokio::test]
    #[ignore]
    async fn test_get_mirakc_event_stream_error() {
        init_test_logging();
        let url = "http://tuner:40772/hoge";
        let result = get_mirakc_event_stream(url, 5).await;
        println!("get_mirakc_event_stream done.");

        // assert!(result.is_err());
        match result {
            Ok(mut stream) => {
                let ev = stream.next().await;
                dbg!(&ev);
                panic!("Expected an error, but got a stream")
            }
            Err(e) => {
                dbg!(&e);
                assert_eq!(
                    e.to_string(),
                    "SSE ストリームの接続中にエラーが発生しました: http request failed with status code 404"
                );
            }
        }
    }
}

#[cfg(test)]
mod mock_tests {
    use super::*;
    use test_util::init_test_logging;
    use tokio::sync::oneshot;
    use warp::Filter;

    fn create_mock_server() -> (String, oneshot::Sender<()>) {
        let (tx, rx) = oneshot::channel();
        let sse_route = warp::path("events").map(move || {
            let stream = futures::stream::iter(vec![Ok::<_, std::convert::Infallible>(
                warp::sse::Event::default()
                    .event("epg.programs-updated")
                    .data("{\"key\":\"value\"}"),
            )]);
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

        let (addr, server) =
            warp::serve(sse_route).bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async {
                rx.await.ok();
            });

        tokio::spawn(server);

        let url = format!("http://{}", addr);
        (url, tx)
    }

    #[tokio::test]
    async fn test_get_mirakc_event_stream_with_mock_server() {
        init_test_logging();

        // Create a mock SSE server
        let (url, tx) = create_mock_server();
        let mut stream = get_mirakc_event_stream(&url, 5).await.unwrap();

        // Test receiving an event
        if let Some(event) = stream.next().await {
            assert_eq!(event.mirakc_url, url);
            assert_eq!(event.event_type, "epg.programs-updated");
            assert_eq!(event.data, "{\"key\":\"value\"}");
        } else {
            panic!("Expected an event, but got none");
        }

        // Shut down the mock server
        let _ = tx.send(());
    }
}
