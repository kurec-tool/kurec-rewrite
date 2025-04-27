use std::sync::Arc;

use domain::{
    model::program::{Channel, Genre, Program},
    ports::ProgramsRetriever,
};
use tracing::{debug, error};

use crate::http_client::{MirakurunGenre, MirakurunProgram, MirakcApiClient};

#[derive(Clone)]
pub struct MirakcProgramsRetriever {
    client: Arc<MirakcApiClient>,
}

impl MirakcProgramsRetriever {
    pub fn new(mirakc_url: &str) -> Self {
        let client = Arc::new(MirakcApiClient::new(mirakc_url));
        Self { client }
    }

    fn convert_program(&self, mirakc_program: MirakurunProgram, service_name: &str) -> Program {
        let genres = mirakc_program.genres
            .map(|genres| self.convert_genres(genres))
            .unwrap_or_default();

        Program {
            id: mirakc_program.id,
            event_id: mirakc_program.event_id,
            service_id: mirakc_program.service_id,
            network_id: mirakc_program.network_id,
            start_at: mirakc_program.start_at,
            duration: mirakc_program.duration,
            is_free: mirakc_program.is_free,
            name: mirakc_program.name,
            description: mirakc_program.description,
            genres,
            channel: Channel {
                id: mirakc_program.service_id as i64,
                name: service_name.to_string(),
            },
        }
    }

    fn convert_genres(&self, mirakc_genres: Vec<MirakurunGenre>) -> Vec<Genre> {
        mirakc_genres
            .into_iter()
            .map(|g| Genre {
                lv1: g.lv1 as u8,
                lv2: g.lv2 as u8,
            })
            .collect()
    }
}

impl ProgramsRetriever for MirakcProgramsRetriever {
    fn get_programs(&self, service_id: i64) -> Vec<Program> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let service_result = self.client.get_programs_by_service(service_id).await;
            
            match service_result {
                Ok(programs) => {
                    let service_name = format!("Service {}", service_id);
                    
                    debug!("Converting {} programs", programs.len());
                    programs
                        .into_iter()
                        .map(|p| self.convert_program(p, &service_name))
                        .collect()
                }
                Err(e) => {
                    error!("Failed to get programs: {:?}", e);
                    Vec::new()
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::MirakurunGenre;

    #[test]
    fn test_convert_genres() {
        let retriever = MirakcProgramsRetriever::new("http://dummy");
        
        let mirakc_genres = vec![
            MirakurunGenre { lv1: 0, lv2: 1, un1: 2, un2: 3 },
            MirakurunGenre { lv1: 4, lv2: 5, un1: 6, un2: 7 },
        ];
        
        let genres = retriever.convert_genres(mirakc_genres);
        
        assert_eq!(genres.len(), 2);
        assert_eq!(genres[0].lv1, 0);
        assert_eq!(genres[0].lv2, 1);
        assert_eq!(genres[1].lv1, 4);
        assert_eq!(genres[1].lv2, 5);
    }
}

#[cfg(test)]
mod mock_tests {
    use super::*;
    use tokio::sync::oneshot;
    use warp::Filter;
    use warp::http::Response;
    use serde_json::json;

    fn create_mock_server() -> (String, oneshot::Sender<()>) {
        let (tx, rx) = oneshot::channel();
        
        let programs_route = warp::path!("services" / i64 / "programs")
            .map(|service_id: i64| {
                let programs = vec![
                    json!({
                        "id": 1,
                        "eventId": 1001,
                        "serviceId": service_id,
                        "networkId": 32736,
                        "startAt": 1619856000000i64,
                        "duration": 1800000,
                        "isFree": true,
                        "name": "テスト番組1",
                        "description": "テスト番組の説明1",
                        "genres": [
                            {
                                "lv1": 7,
                                "lv2": 15,
                                "un1": 0,
                                "un2": 0
                            }
                        ]
                    }),
                    json!({
                        "id": 2,
                        "eventId": 1002,
                        "serviceId": service_id,
                        "networkId": 32736,
                        "startAt": 1619857800000i64,
                        "duration": 1800000,
                        "isFree": true,
                        "name": "テスト番組2",
                        "description": "テスト番組の説明2",
                        "genres": [
                            {
                                "lv1": 8,
                                "lv2": 0,
                                "un1": 0,
                                "un2": 0
                            }
                        ]
                    })
                ];
                
                Response::builder()
                    .header("content-type", "application/json")
                    .body(serde_json::to_string(&programs).unwrap())
            });
        
        let (addr, server) =
            warp::serve(programs_route).bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async {
                rx.await.ok();
            });
        
        tokio::spawn(server);
        
        let url = format!("http://{}", addr);
        (url, tx)
    }

    #[tokio::test]
    async fn test_get_programs_success() {
        let (url, tx) = create_mock_server();
        
        async fn get_programs_async(url: &str, service_id: i64) -> Vec<Program> {
            let client = Arc::new(MirakcApiClient::new(url));
            let service_result = client.get_programs_by_service(service_id).await;
            
            match service_result {
                Ok(programs) => {
                    let service_name = format!("Service {}", service_id);
                    
                    programs
                        .into_iter()
                        .map(|p| {
                            let genres = p.genres
                                .map(|genres| genres
                                    .into_iter()
                                    .map(|g| Genre {
                                        lv1: g.lv1 as u8,
                                        lv2: g.lv2 as u8,
                                    })
                                    .collect())
                                .unwrap_or_default();
                            
                            Program {
                                id: p.id,
                                event_id: p.event_id,
                                service_id: p.service_id,
                                network_id: p.network_id,
                                start_at: p.start_at,
                                duration: p.duration,
                                is_free: p.is_free,
                                name: p.name,
                                description: p.description,
                                genres,
                                channel: Channel {
                                    id: p.service_id as i64,
                                    name: service_name.clone(),
                                },
                            }
                        })
                        .collect()
                }
                Err(e) => {
                    eprintln!("Failed to get programs: {:?}", e);
                    Vec::new()
                }
            }
        }
        
        let programs = get_programs_async(&url, 1).await;
        
        assert_eq!(programs.len(), 2);
        assert_eq!(programs[0].id, 1);
        assert_eq!(programs[0].name, Some("テスト番組1".to_string()));
        assert_eq!(programs[0].genres.len(), 1);
        assert_eq!(programs[0].genres[0].lv1, 7);
        assert_eq!(programs[0].genres[0].lv2, 15);
        
        let _ = tx.send(());
    }
}
