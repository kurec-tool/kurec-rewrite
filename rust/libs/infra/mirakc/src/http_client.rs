use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::Deserialize;
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

    pub async fn get_programs_by_service(&self, service_id: i64) -> Result<Vec<MirakurunProgram>, MirakcApiError> {
        let url = format!("{}/services/{}/programs", self.base_url, service_id);
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
                Err(MirakcApiError::UnknownError(format!("Unexpected status code: {}", status)))
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
    pub genres: Option<Vec<MirakurunGenre>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MirakurunGenre {
    pub lv1: i32,
    pub lv2: i32,
    pub un1: i32,
    pub un2: i32,
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
