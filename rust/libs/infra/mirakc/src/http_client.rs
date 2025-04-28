use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error};

#[derive(Error, Debug)]
pub enum MirakcApiError {
    #[error("HTTP リクエストエラー: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("サービス(ID={0})が見つかりません")]
    ServiceNotFound(i64),
    #[error("不明なエラー: {0}")]
    UnknownError(String),
}

#[derive(Clone, Debug)]
pub struct MirakcApiClient {
    base_url: String,
    client: Client,
}

impl MirakcApiClient {
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            base_url: base_url.to_string(),
            client,
        }
    }

    pub async fn get_programs_by_service(
        &self,
        service_id: i64,
    ) -> Result<Vec<MirakurunProgram>, MirakcApiError> {
        let url = format!("{}/api/services/{}/programs", self.base_url, service_id);
        debug!("Fetching programs from: {}", url);

        let response = self.client.get(&url).send().await?;

        match response.status() {
            StatusCode::OK => {
                let programs = response.json::<Vec<MirakurunProgram>>().await?;
                debug!("Got {} programs for service {}", programs.len(), service_id);
                Ok(programs)
            }
            StatusCode::NOT_FOUND => {
                error!("Service not found: {}", service_id);
                Err(MirakcApiError::ServiceNotFound(service_id))
            }
            status => {
                error!("Unexpected status code: {}", status);
                Err(MirakcApiError::UnknownError(format!(
                    "Unexpected status code: {}",
                    status
                )))
            }
        }
    }

    pub async fn get_service(&self, service_id: i64) -> Result<MirakurunService, MirakcApiError> {
        let url = format!("{}/api/services/{}", self.base_url, service_id);
        debug!("Fetching service from: {}", url);

        let response = self.client.get(&url).send().await?;

        match response.status() {
            StatusCode::OK => {
                let service = response.json::<MirakurunService>().await?;
                debug!("Got service: {}", service.name);
                Ok(service)
            }
            StatusCode::NOT_FOUND => {
                error!("Service not found: {}", service_id);
                Err(MirakcApiError::ServiceNotFound(service_id))
            }
            status => {
                error!("Unexpected status code: {}", status);
                Err(MirakcApiError::UnknownError(format!(
                    "Unexpected status code: {}",
                    status
                )))
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MirakurunProgram {
    pub id: i64,
    #[serde(rename = "eventId")]
    pub event_id: i32,
    #[serde(rename = "serviceId")]
    pub service_id: i32,
    #[serde(rename = "networkId")]
    pub network_id: i32,
    #[serde(rename = "startAt")]
    pub start_at: i64,
    pub duration: i64,
    #[serde(rename = "isFree")]
    pub is_free: bool,
    pub name: Option<String>,
    pub description: Option<String>,
    pub extended: Option<BTreeMap<String, String>>,
    pub video: Option<MirakurunVideo>,
    pub audio: Option<MirakurunAudio>,
    pub audios: Option<Vec<MirakurunAudio>>,
    pub genres: Option<Vec<MirakurunGenre>>,
    #[serde(rename = "relatedItems")]
    pub related_items: Option<Vec<MirakurunRelatedItem>>,
}

impl MirakurunProgram {
    pub fn get_extended_description(&self) -> Option<String> {
        self.extended.as_ref().map(|extended| {
            extended
                .iter()
                .map(|(key, value)| format!("{}：{}", key, value))
                .collect::<Vec<String>>()
                .join("\n")
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MirakurunGenre {
    pub lv1: i32,
    pub lv2: i32,
    pub un1: i32,
    pub un2: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MirakurunVideo {
    pub r#type: Option<String>,
    pub resolution: Option<String>,
    #[serde(rename = "streamContent")]
    pub stream_content: Option<u8>,
    #[serde(rename = "componentType")]
    pub component_type: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MirakurunAudio {
    #[serde(rename = "componentType")]
    pub component_type: Option<u8>,
    #[serde(rename = "isMain")]
    pub is_main: Option<bool>,
    #[serde(rename = "samplingRate")]
    pub sampling_rate: Option<u32>,
    pub langs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MirakurunRelatedItem {
    pub r#type: String,
    #[serde(rename = "networkId")]
    pub network_id: Option<i32>,
    #[serde(rename = "serviceId")]
    pub service_id: i32,
    #[serde(rename = "eventId")]
    pub event_id: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MirakurunService {
    pub id: i64,
    #[serde(rename = "serviceId")]
    pub service_id: i32,
    #[serde(rename = "networkId")]
    pub network_id: i32,
    #[serde(rename = "type")]
    pub service_type: i32,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::oneshot;
    use warp::Filter;
    use warp::http::Response;

    fn create_mock_server() -> (String, oneshot::Sender<()>) {
        let (tx, rx) = oneshot::channel();

        let service_route = warp::path!("api" / "services" / i64).map(|service_id: i64| {
            if service_id == 1 || service_id == 23608 {
                let service = json!({
                    "id": service_id,
                    "serviceId": 23608,
                    "networkId": 32391,
                    "type": 1,
                    "name": "テストチャンネル"
                });

                Response::builder()
                    .header("content-type", "application/json")
                    .body(serde_json::to_string(&service).unwrap())
            } else {
                Response::builder()
                    .status(404)
                    .body("Not Found".to_string())
            }
        });

        let programs_route =
            warp::path!("api" / "services" / i64 / "programs").map(|service_id: i64| {
                let programs = vec![json!({
                    "id": 1,
                    "eventId": 1001,
                    "serviceId": service_id,
                    "networkId": 1,
                    "startAt": 1619856000000i64,
                    "duration": 1800000,
                    "isFree": true,
                    "name": "テスト番組1",
                    "description": "テスト番組の説明1",
                    "extended": {
                        "概要": "テスト番組の概要です",
                        "出演者": "テスト出演者1\nテスト出演者2"
                    },
                    "video": {
                        "type": "mpeg2",
                        "resolution": "1080i",
                        "streamContent": 1,
                        "componentType": 179
                    },
                    "audio": {
                        "componentType": 3,
                        "isMain": true,
                        "samplingRate": 48000,
                        "langs": [
                            "jpn"
                        ]
                    },
                    "audios": [
                        {
                            "componentType": 3,
                            "isMain": true,
                            "samplingRate": 48000,
                            "langs": [
                                "jpn"
                            ]
                        }
                    ],
                    "genres": [
                        {
                            "lv1": 7,
                            "lv2": 0,
                            "un1": 15,
                            "un2": 15
                        }
                    ],
                    "relatedItems": [
                        {
                            "type": "shared",
                            "networkId": null,
                            "serviceId": 1,
                            "eventId": 1001
                        },
                        {
                            "type": "shared",
                            "networkId": null,
                            "serviceId": 2,
                            "eventId": 1001
                        }
                    ]
                })];

                Response::builder()
                    .header("content-type", "application/json")
                    .body(serde_json::to_string(&programs).unwrap())
            });

        let routes = service_route.or(programs_route);

        let (addr, server) =
            warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async {
                rx.await.ok();
            });

        tokio::spawn(server);

        let url = format!("http://{}", addr);
        (url, tx)
    }

    #[tokio::test]
    async fn test_get_service() {
        let (url, tx) = create_mock_server();
        let client = MirakcApiClient::new(&url);

        let service = client.get_service(1).await.unwrap();

        assert_eq!(service.id, 1);
        assert_eq!(service.service_id, 23608);
        assert_eq!(service.network_id, 32391);
        assert_eq!(service.name, "テストチャンネル");

        let _ = tx.send(());
    }

    #[tokio::test]
    async fn test_get_programs_by_service() {
        let (url, tx) = create_mock_server();
        let client = MirakcApiClient::new(&url);

        let programs = client.get_programs_by_service(1).await.unwrap();

        assert_eq!(programs.len(), 1);
        let program = &programs[0];

        assert_eq!(program.id, 1);
        assert_eq!(program.event_id, 1001);
        assert_eq!(program.service_id, 1);
        assert_eq!(program.network_id, 1);
        assert_eq!(program.start_at, 1619856000000);
        assert_eq!(program.duration, 1800000);
        assert_eq!(program.is_free, true);
        assert_eq!(program.name, Some("テスト番組1".to_string()));
        assert_eq!(program.description, Some("テスト番組の説明1".to_string()));

        assert!(program.extended.is_some());
        let extended = program.extended.as_ref().unwrap();
        assert_eq!(extended.len(), 2);
        assert!(extended.contains_key("概要"));
        assert!(extended.contains_key("出演者"));

        let extended_desc = program.get_extended_description().unwrap();
        assert!(extended_desc.contains("概要："));
        assert!(extended_desc.contains("出演者："));

        assert!(program.video.is_some());
        let video = program.video.as_ref().unwrap();
        assert_eq!(video.r#type, Some("mpeg2".to_string()));
        assert_eq!(video.resolution, Some("1080i".to_string()));
        assert_eq!(video.component_type, Some(179));

        assert!(program.audio.is_some());
        let audio = program.audio.as_ref().unwrap();
        assert_eq!(audio.component_type, Some(3));
        assert_eq!(audio.is_main, Some(true));
        assert_eq!(audio.sampling_rate, Some(48000));
        assert_eq!(audio.langs, Some(vec!["jpn".to_string()]));

        assert!(program.genres.is_some());
        let genres = program.genres.as_ref().unwrap();
        assert_eq!(genres.len(), 1);
        assert_eq!(genres[0].lv1, 7);
        assert_eq!(genres[0].lv2, 0);

        assert!(program.related_items.is_some());
        let related_items = program.related_items.as_ref().unwrap();
        assert_eq!(related_items.len(), 2);
        assert_eq!(related_items[0].r#type, "shared");
        assert_eq!(related_items[0].service_id, 1);
        assert_eq!(related_items[0].event_id, 1001);

        let _ = tx.send(());
    }

    #[tokio::test]
    async fn test_service_not_found() {
        let (url, tx) = create_mock_server();
        let client = MirakcApiClient::new(&url);

        let result = client.get_service(999999).await;

        assert!(result.is_err());

        let _ = tx.send(());
    }
}
