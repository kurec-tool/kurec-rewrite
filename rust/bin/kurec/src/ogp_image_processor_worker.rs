use domain::{
    model::event::ogp,
    usecase::{OgpImageProcessorUseCase, OgpImageProcessorUseCaseImpl, WebpImageData},
};
use nats::stream::{EventReader, EventStore};
use http::ReqwestImageFetcher;
use nats::nats::NatsClient;
use tracing::{debug, error, info};

use crate::repositories::WebpImageDataRepository;

pub async fn process_ogp_image_processor(nats_client: NatsClient) {
    debug!("OGP画像処理ワーカーを開始します...");

    let image_request_store = EventStore::<ogp::url::ImageRequest>::new(nats_client.clone())
        .await
        .unwrap();

    let webp_image_repository = WebpImageDataRepository::new(nats_client.clone())
        .await
        .unwrap();

    let image_fetcher = ReqwestImageFetcher::default();
    let image_processor = domain::service::WebpImageProcessor::default();

    let usecase =
        OgpImageProcessorUseCaseImpl::new(image_fetcher, image_processor, webp_image_repository);

    let reader = image_request_store
        .get_reader("ogp_image_processor".to_string())
        .await
        .unwrap();

    debug!("画像リクエストイベント待機中...");

    loop {
        match reader.next().await {
            Ok((event, mut ack_handle)) => {
                let url = &event.url;
                info!("画像リクエストイベントを受信: url={}", url);

                match usecase.process_image_request(&event).await {
                    Ok(_) => {
                        info!("画像を正常に処理しました: url={}", url);
                    }
                    Err(e) => {
                        error!("画像の処理に失敗しました: url={}, error={:?}", url, e);
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
