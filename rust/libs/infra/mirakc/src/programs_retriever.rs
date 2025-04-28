use std::sync::Arc;

use domain::{
    error::DomainError,
    model::program::{
        Audio, Channel, Genre, Program, ProgramIdentifiers, ProgramTiming, RelatedItem, Video,
    },
    ports::ProgramsRetriever,
};
use tracing::{debug, error};

use crate::http_client::{
    MirakcApiClient, MirakcApiError, MirakurunAudio, MirakurunGenre, MirakurunProgram,
    MirakurunRelatedItem, MirakurunVideo,
};

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
        let genres = mirakc_program
            .genres
            .clone()
            .map(|genres| self.convert_genres(genres))
            .unwrap_or_default();

        let video = mirakc_program.video.clone().map(|v| self.convert_video(v));
        let audio = mirakc_program.audio.clone().map(|a| self.convert_audio(a));
        let related_items = mirakc_program
            .related_items
            .clone()
            .map(|items| self.convert_related_items(items));

        let mut program = Program::new(
            ProgramIdentifiers {
                id: mirakc_program.id,
                event_id: mirakc_program.event_id,
                network_id: mirakc_program.network_id,
                service_id: mirakc_program.service_id,
            },
            ProgramTiming {
                start_at: mirakc_program.start_at,
                duration: mirakc_program.duration,
            },
            mirakc_program.is_free,
            mirakc_program.name.clone(),
            mirakc_program.description.clone(),
            genres.clone(),
            Channel {
                id: mirakc_program.service_id as i64,
                name: service_name.to_string(),
            },
        );

        program.extended = mirakc_program.extended.clone();
        program.extended_description = mirakc_program.get_extended_description();

        program.video = video;

        program.audio = audio;

        program.related_items = related_items;

        program
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

    fn convert_video(&self, mirakc_video: MirakurunVideo) -> Video {
        let component_type_name = mirakc_video.component_type.map(|ct| match ct {
            0x01 => "480i(525i), アスペクト比4:3".to_string(),
            0x02 => "480i(525i), アスペクト比16:9 パンベクトルあり".to_string(),
            0x03 => "480i(525i), アスペクト比16:9 パンベクトルなし".to_string(),
            0x04 => "480i(525i), アスペクト比 > 16:9".to_string(),
            0x83 => "4320p, アスペクト比16:9".to_string(),
            0x91 => "2160p, アスペクト比4:3".to_string(),
            0x92 => "2160p, アスペクト比16:9 パンベクトルあり".to_string(),
            0x93 => "2160p, アスペクト比16:9 パンベクトルなし".to_string(),
            0x94 => "2160p, アスペクト比 > 16:9".to_string(),
            0xa1 => "480p(525p), アスペクト比4:3".to_string(),
            0xa2 => "480p(525p), アスペクト比16:9 パンベクトルあり".to_string(),
            0xa3 => "480p(525p), アスペクト比16:9 パンベクトルなし".to_string(),
            0xa4 => "480p(525p), アスペクト比 > 16:9".to_string(),
            0xb1 => "1080i(1125i), アスペクト比4:3".to_string(),
            0xb2 => "1080i(1125i), アスペクト比16:9 パンベクトルあり".to_string(),
            0xb3 => "1080i(1125i), アスペクト比16:9 パンベクトルなし".to_string(),
            0xb4 => "1080i(1125i), アスペクト比 > 16:9".to_string(),
            0xc1 => "720p(750p), アスペクト比4:3".to_string(),
            0xc2 => "720p(750p), アスペクト比16:9 パンベクトルあり".to_string(),
            0xc3 => "720p(750p), アスペクト比16:9 パンベクトルなし".to_string(),
            0xc4 => "720p(750p), アスペクト比 > 16:9".to_string(),
            0xd1 => "240p アスペクト比4:3".to_string(),
            0xd2 => "240p アスペクト比16:9 パンベクトルあり".to_string(),
            0xd3 => "240p アスペクト比16:9 パンベクトルなし".to_string(),
            0xd4 => "240p アスペクト比 > 16:9".to_string(),
            0xe1 => "1080p(1125p), アスペクト比4:3".to_string(),
            0xe2 => "1080p(1125p), アスペクト比16:9 パンベクトルあり".to_string(),
            0xe3 => "1080p(1125p), アスペクト比16:9 パンベクトルなし".to_string(),
            0xe4 => "1080p(1125p), アスペクト比 > 16:9".to_string(),
            0xf1 => "180p アスペクト比4:3".to_string(),
            0xf2 => "180p アスペクト比16:9 パンベクトルあり".to_string(),
            0xf3 => "180p アスペクト比16:9 パンベクトルなし".to_string(),
            0xf4 => "180p アスペクト比 > 16:9".to_string(),
            _ => format!("不明なコンポーネントタイプ: {}", ct),
        });

        Video {
            r#type: mirakc_video.r#type,
            resolution: mirakc_video.resolution,
            stream_content: mirakc_video.stream_content,
            component_type: mirakc_video.component_type,
            component_type_name,
        }
    }

    fn convert_audio(&self, mirakc_audio: MirakurunAudio) -> Audio {
        let component_type_name = mirakc_audio.component_type.map(|ct| match ct {
            0b00000 => "将来使用のためリザーブ".to_string(),
            0b00001 => "1/0モード(シングルモノ)".to_string(),
            0b00010 => "1/0 + 1/0モード(デュアルモノ)".to_string(),
            0b00011 => "2/0モード(ステレオ)".to_string(),
            0b00100 => "2/1モード".to_string(),
            0b00101 => "3/0モード".to_string(),
            0b00110 => "2/2モード".to_string(),
            0b00111 => "3/1モード".to_string(),
            0b01000 => "3/2モード".to_string(),
            0b01001 => "3/2 + LFEモード(3/2.1モード)".to_string(),
            0b01010 => "3/3.1モード".to_string(),
            0b01011 => "2/0/0-2/0/2-0.1モード".to_string(),
            0b01100 => "5/2.1モード".to_string(),
            0b01101 => "3/2/2.1モード".to_string(),
            0b01110 => "2/0/0-3/0/2-0.1モード".to_string(),
            0b01111 => "0/2/0-3/0/2-0.1モード".to_string(),
            0b10000 => "2/0/0-3/2/3-0.2モード".to_string(),
            0b10001 => "3/3/3-5/2/3-3/0/0.2モード".to_string(),
            _ => format!("不明なコンポーネントタイプ: {}", ct),
        });

        let sampling_rate_name = mirakc_audio.sampling_rate.map(|sr| match sr {
            16000 => "16kHz".to_string(),
            22050 => "22.05kHz".to_string(),
            24000 => "24kHz".to_string(),
            32000 => "32kHz".to_string(),
            44100 => "44.1kHz".to_string(),
            48000 => "48kHz".to_string(),
            _ => format!("{}Hz", sr),
        });

        Audio {
            component_type: mirakc_audio.component_type,
            component_type_name,
            is_main: mirakc_audio.is_main,
            sampling_rate: mirakc_audio.sampling_rate,
            sampling_rate_name,
            langs: mirakc_audio.langs,
        }
    }

    fn convert_related_items(&self, mirakc_items: Vec<MirakurunRelatedItem>) -> Vec<RelatedItem> {
        mirakc_items
            .into_iter()
            .map(|item| RelatedItem {
                r#type: item.r#type,
                network_id: item.network_id,
                service_id: item.service_id,
                event_id: item.event_id,
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl ProgramsRetriever for MirakcProgramsRetriever {
    async fn get_programs(&self, service_id: i64) -> Result<Vec<Program>, DomainError> {
        let service_result = self.client.get_service(service_id).await;
        let service_name = match service_result {
            Ok(service) => service.name,
            Err(e) => {
                if let MirakcApiError::ServiceNotFound(_) = e {
                    return Err(DomainError::ServiceNotFound(service_id));
                }
                return Err(DomainError::ProgramsRetrievalError(format!(
                    "サービス情報の取得に失敗: {}",
                    e
                )));
            }
        };

        let programs_result = self.client.get_programs_by_service(service_id).await;

        match programs_result {
            Ok(programs) => {
                debug!("Converting {} programs", programs.len());
                Ok(programs
                    .into_iter()
                    .map(|p| self.convert_program(p, &service_name))
                    .collect())
            }
            Err(e) => {
                error!("Failed to get programs: {:?}", e);
                Err(DomainError::ProgramsRetrievalError(format!(
                    "プログラム情報の取得に失敗: {}",
                    e
                )))
            }
        }
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
            MirakurunGenre {
                lv1: 0,
                lv2: 1,
                un1: 2,
                un2: 3,
            },
            MirakurunGenre {
                lv1: 4,
                lv2: 5,
                un1: 6,
                un2: 7,
            },
        ];

        let genres = retriever.convert_genres(mirakc_genres);

        assert_eq!(genres.len(), 2);
        assert_eq!(genres[0].lv1, 0);
        assert_eq!(genres[0].lv2, 1);
        assert_eq!(genres[1].lv1, 4);
        assert_eq!(genres[1].lv2, 5);
    }

    #[test]
    fn test_convert_video() {
        let retriever = MirakcProgramsRetriever::new("http://dummy");

        let mirakc_video = MirakurunVideo {
            r#type: Some("mpeg2".to_string()),
            resolution: Some("1080i".to_string()),
            stream_content: Some(1),
            component_type: Some(0xb3),
        };

        let video = retriever.convert_video(mirakc_video);

        assert_eq!(video.r#type, Some("mpeg2".to_string()));
        assert_eq!(video.resolution, Some("1080i".to_string()));
        assert_eq!(video.component_type, Some(0xb3));
        assert_eq!(
            video.component_type_name,
            Some("1080i(1125i), アスペクト比16:9 パンベクトルなし".to_string())
        );
    }

    #[test]
    fn test_convert_audio() {
        let retriever = MirakcProgramsRetriever::new("http://dummy");

        let mirakc_audio = MirakurunAudio {
            component_type: Some(3),
            is_main: Some(true),
            sampling_rate: Some(48000),
            langs: Some(vec!["jpn".to_string()]),
        };

        let audio = retriever.convert_audio(mirakc_audio);

        assert_eq!(audio.component_type, Some(3));
        assert_eq!(
            audio.component_type_name,
            Some("2/0モード(ステレオ)".to_string())
        );
        assert_eq!(audio.is_main, Some(true));
        assert_eq!(audio.sampling_rate, Some(48000));
        assert_eq!(audio.sampling_rate_name, Some("48kHz".to_string()));
        assert_eq!(audio.langs, Some(vec!["jpn".to_string()]));
    }
}

#[cfg(test)]
mod mock_tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::oneshot;
    use warp::Filter;
    use warp::http::Response;

    fn create_mock_server() -> (String, oneshot::Sender<()>) {
        let (tx, rx) = oneshot::channel();

        let service_route = warp::path!("services" / i64).map(|service_id: i64| {
            let service = json!({
                "id": service_id,
                "serviceId": service_id,
                "networkId": 32736,
                "type": 1,
                "name": "テストチャンネル"
            });

            Response::builder()
                .header("content-type", "application/json")
                .body(serde_json::to_string(&service).unwrap())
        });

        let programs_route = warp::path!("services" / i64 / "programs")
            .map(|service_id: i64| {
                let programs = vec![
                    json!({
                        "id": 323912360808478i64,
                        "eventId": 8478,
                        "serviceId": service_id,
                        "networkId": 32736,
                        "startAt": 1619856000000i64,
                        "duration": 1800000,
                        "isFree": true,
                        "name": "小林さんちのメイドラゴン　＃３[再]",
                        "description": "＃３「新生活、はじまる！（もちろんうまくいきません）」",
                        "extended": {
                            "あらすじ◇": "トールに加えカンナも住むようになった小林さんち。賑やかになったのはいいが、いかんせん狭いマンションに三人暮らしは窮屈。そこで引っ越しを決意する小林さん。引っ越した先で滝谷を呼んでパーティーを開くことになったのだが、そこにトールの知り合いである洞窟住まいのファフニールや、太古からこちらの世界に住まうルコアなど、新しいドラゴンたちが小林さんちを訪ねてくる……。",
                            "出演者": "【小林】\n田村睦心\n【トール】\n桑原由気\n【カンナ】\n長縄まりあ\n【エルマ】\n高田憂希\n【ルコア】\n高橋未奈美\n【ファフニール】\n小野大輔\n【滝谷真】\n中村悠一\n【才川リコ】\n加藤英美里\n【才川ジョージー】\n後藤邑子\n【真ヶ土翔太】\n石原夏織"
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
                                "serviceId": 23608,
                                "eventId": 8478
                            }
                        ]
                    })
                ];

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
    async fn test_get_programs_success() {
        let (url, tx) = create_mock_server();
        let retriever = MirakcProgramsRetriever::new(&url);

        let programs = retriever.get_programs(1).await.unwrap();

        assert_eq!(programs.len(), 1);
        let program = &programs[0];

        assert_eq!(program.id, 323912360808478);
        assert_eq!(program.event_id, 8478);
        assert_eq!(program.service_id, 1);
        assert_eq!(program.network_id, 32736);
        assert_eq!(program.start_at, 1619856000000);
        assert_eq!(program.duration, 1800000);
        assert_eq!(program.end_at, program.start_at + program.duration);
        assert_eq!(program.is_free, true);
        assert_eq!(
            program.name,
            Some("小林さんちのメイドラゴン　＃３[再]".to_string())
        );
        assert_eq!(
            program.description,
            Some("＃３「新生活、はじまる！（もちろんうまくいきません）」".to_string())
        );

        assert_eq!(program.channel.id, 1);
        assert_eq!(program.channel.name, "テストチャンネル");

        assert_eq!(program.genres.len(), 1);
        assert_eq!(program.genres[0].lv1, 7);
        assert_eq!(program.genres[0].lv2, 0);
        assert_eq!(program.genre_names.len(), 1);
        assert_eq!(program.genre_names[0], "アニメ・特撮/国内アニメ");

        assert!(program.extended.is_some());
        let extended = program.extended.as_ref().unwrap();
        assert_eq!(extended.len(), 2);
        assert!(extended.contains_key("あらすじ◇"));
        assert!(extended.contains_key("出演者"));

        assert!(program.extended_description.is_some());
        let extended_desc = program.extended_description.as_ref().unwrap();
        assert!(extended_desc.contains("あらすじ◇："));
        assert!(extended_desc.contains("出演者："));

        assert!(program.video.is_some());
        let video = program.video.as_ref().unwrap();
        assert_eq!(video.r#type, Some("mpeg2".to_string()));
        assert_eq!(video.resolution, Some("1080i".to_string()));
        assert_eq!(video.component_type, Some(179));
        assert!(video.component_type_name.is_some());

        assert!(program.audio.is_some());
        let audio = program.audio.as_ref().unwrap();
        assert_eq!(audio.component_type, Some(3));
        assert_eq!(
            audio.component_type_name,
            Some("2/0モード(ステレオ)".to_string())
        );
        assert_eq!(audio.is_main, Some(true));
        assert_eq!(audio.sampling_rate, Some(48000));
        assert_eq!(audio.sampling_rate_name, Some("48kHz".to_string()));
        assert_eq!(audio.langs, Some(vec!["jpn".to_string()]));

        assert!(program.related_items.is_some());
        let related_items = program.related_items.as_ref().unwrap();
        assert_eq!(related_items.len(), 1);
        assert_eq!(related_items[0].r#type, "shared");
        assert_eq!(related_items[0].service_id, 23608);
        assert_eq!(related_items[0].event_id, 8478);

        let _ = tx.send(());
    }
}
