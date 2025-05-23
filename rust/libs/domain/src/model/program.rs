use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub id: i64,
    pub event_id: i32,
    pub service_id: i32,
    pub network_id: i32,
    pub start_at: i64,
    pub duration: i64,
    pub end_at: i64,
    pub is_free: bool,
    pub name: Option<String>,
    pub description: Option<String>,
    pub extended: Option<BTreeMap<String, String>>,
    pub extended_description: Option<String>,
    pub genres: Vec<Genre>,
    pub genre_names: Vec<String>,
    pub channel: Channel,
    pub video: Option<Video>,
    pub audio: Option<Audio>,
    pub related_items: Option<Vec<RelatedItem>>,
}

impl Program {
    pub fn new(
        identifiers: ProgramIdentifiers,
        timing: ProgramTiming,
        is_free: bool,
        name: Option<String>,
        description: Option<String>,
        genres: Vec<Genre>,
        channel: Channel,
    ) -> Self {
        let end_at = timing.start_at + timing.duration;

        Self {
            id: identifiers.id,
            event_id: identifiers.event_id,
            service_id: identifiers.service_id,
            network_id: identifiers.network_id,
            start_at: timing.start_at,
            duration: timing.duration,
            end_at,
            is_free,
            name,
            description,
            extended: None,
            extended_description: None,
            genres: genres.clone(),
            genre_names: genres.iter().map(|g| g.to_string()).collect(),
            channel,
            video: None,
            audio: None,
            related_items: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgramIdentifiers {
    pub id: i64,
    pub event_id: i32,
    pub service_id: i32,
    pub network_id: i32,
}

#[derive(Debug, Clone)]
pub struct ProgramTiming {
    pub start_at: i64,
    pub duration: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genre {
    pub lv1: u8,
    pub lv2: u8,
}

use std::fmt;

impl fmt::Display for Genre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let genre_name = match self.lv1 {
            0 => "ニュース・報道",
            1 => "スポーツ",
            2 => "情報・ワイドショー",
            3 => "ドラマ",
            4 => "音楽",
            5 => "バラエティ",
            6 => "映画",
            7 => "アニメ・特撮",
            8 => "ドキュメンタリー・教養",
            9 => "劇場・公演",
            10 => "趣味・教育",
            11 => "福祉",
            12 => "予備",
            13 => "予備",
            14 => "拡張",
            15 => "その他",
            _ => "不明",
        };

        let sub_genre_name = match (self.lv1, self.lv2) {
            (0, 0) => "定時・総合",
            (0, 1) => "天気",
            (0, 2) => "特集・ドキュメント",
            (0, 3) => "政治・国会",
            (0, 4) => "経済・市況",
            (0, 5) => "海外・国際",
            (0, 6) => "解説",
            (0, 7) => "討論・会談",
            (0, 8) => "報道特番",
            (0, 9) => "ローカル・地域",
            (0, 10) => "交通",
            (0, 15) => "その他",

            (1, 0) => "スポーツニュース",
            (1, 1) => "野球",
            (1, 2) => "サッカー",
            (1, 3) => "ゴルフ",
            (1, 4) => "その他の球技",
            (1, 5) => "相撲・格闘技",
            (1, 6) => "オリンピック・国際大会",
            (1, 7) => "マラソン・陸上・水泳",
            (1, 8) => "モータースポーツ",
            (1, 9) => "マリン・ウィンタースポーツ",
            (1, 10) => "競馬・公営競技",
            (1, 15) => "その他",

            (2, 0) => "芸能・ワイドショー",
            (2, 1) => "ファッション",
            (2, 2) => "暮らし・住まい",
            (2, 3) => "健康・医療",
            (2, 4) => "ショッピング・通販",
            (2, 5) => "グルメ・料理",
            (2, 6) => "イベント",
            (2, 7) => "番組紹介・お知らせ",
            (2, 15) => "その他",

            (3, 0) => "国内ドラマ",
            (3, 1) => "海外ドラマ",
            (3, 2) => "時代劇",
            (3, 15) => "その他",

            (4, 0) => "国内ロック・ポップス",
            (4, 1) => "海外ロック・ポップス",
            (4, 2) => "クラシック・オペラ",
            (4, 3) => "ジャズ・フュージョン",
            (4, 4) => "歌謡曲・演歌",
            (4, 5) => "ライブ・コンサート",
            (4, 6) => "ランキング・リクエスト",
            (4, 7) => "カラオケ・のど自慢",
            (4, 8) => "民謡・邦楽",
            (4, 9) => "童謡・キッズ",
            (4, 10) => "民族音楽・ワールドミュージック",
            (4, 15) => "その他",

            (5, 0) => "クイズ",
            (5, 1) => "ゲーム",
            (5, 2) => "トークバラエティ",
            (5, 3) => "お笑い・コメディ",
            (5, 4) => "音楽バラエティ",
            (5, 5) => "旅バラエティ",
            (5, 6) => "料理バラエティ",
            (5, 15) => "その他",

            (6, 0) => "洋画",
            (6, 1) => "邦画",
            (6, 2) => "アニメ",
            (6, 15) => "その他",

            (7, 0) => "国内アニメ",
            (7, 1) => "海外アニメ",
            (7, 2) => "特撮",
            (7, 15) => "その他",

            (8, 0) => "社会・時事",
            (8, 1) => "歴史・紀行",
            (8, 2) => "自然・動物・環境",
            (8, 3) => "宇宙・科学・医学",
            (8, 4) => "カルチャー・伝統文化",
            (8, 5) => "文学・文芸",
            (8, 6) => "スポーツ",
            (8, 7) => "ドキュメンタリー全般",
            (8, 8) => "インタビュー・討論",
            (8, 15) => "その他",

            (9, 0) => "現代劇・新劇",
            (9, 1) => "ミュージカル",
            (9, 2) => "ダンス・バレエ",
            (9, 3) => "落語・演芸",
            (9, 4) => "歌舞伎・古典",
            (9, 15) => "その他",

            (10, 0) => "旅・釣り・アウトドア",
            (10, 1) => "園芸・ペット・手芸",
            (10, 2) => "音楽・美術・工芸",
            (10, 3) => "囲碁・将棋",
            (10, 4) => "麻雀・パチンコ",
            (10, 5) => "車・オートバイ",
            (10, 6) => "コンピュータ・TVゲーム",
            (10, 7) => "会話・語学",
            (10, 8) => "幼児・小学生",
            (10, 9) => "中学生・高校生",
            (10, 10) => "大学生・受験",
            (10, 11) => "生涯教育・資格",
            (10, 12) => "教育問題",
            (10, 15) => "その他",

            (11, 0) => "高齢者",
            (11, 1) => "障害者",
            (11, 2) => "社会福祉",
            (11, 3) => "ボランティア",
            (11, 4) => "手話",
            (11, 5) => "文字(字幕)",
            (11, 6) => "音声解説",
            (11, 15) => "その他",

            (15, 15) => "その他",

            _ => "",
        };

        if !sub_genre_name.is_empty() {
            write!(f, "{}/{}", genre_name, sub_genre_name)
        } else {
            write!(f, "{}", genre_name)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Video {
    pub r#type: Option<String>,
    pub resolution: Option<String>,
    pub stream_content: Option<u8>,
    pub component_type: Option<u8>,
    pub component_type_name: Option<String>,
}

impl Video {
    pub fn get_component_type_name(component_type: u8) -> String {
        match component_type {
            0xb1 => "480i(525i), アスペクト比4:3 パンベクトルなし".to_string(),
            0xb2 => "480i(525i), アスペクト比16:9 パンベクトルあり".to_string(),
            0xb3 => "1080i(1125i), アスペクト比16:9 パンベクトルなし".to_string(),
            0xb4 => "720p(750p), アスペクト比16:9 パンベクトルなし".to_string(),
            0xc1 => "480i(525i), アスペクト比4:3 パンベクトルなし".to_string(),
            0xc3 => "720p(750p), アスペクト比16:9 パンベクトルなし".to_string(),
            0xc4 => "240p アスペクト比4:3 パンベクトルなし".to_string(),
            0xd1 => "1080i(1125i), アスペクト比4:3 パンベクトルなし".to_string(),
            0xd2 => "1080i(1125i), アスペクト比16:9 パンベクトルあり".to_string(),
            0xd3 => "2160p(2160p), アスペクト比16:9 パンベクトルなし".to_string(),
            _ => format!("不明なコンポーネントタイプ: 0x{:x}", component_type),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audio {
    pub component_type: Option<u8>,
    pub component_type_name: Option<String>,
    pub is_main: Option<bool>,
    pub sampling_rate: Option<u32>,
    pub sampling_rate_name: Option<String>,
    pub langs: Option<Vec<String>>,
}

impl Audio {
    pub fn get_component_type_name(component_type: u8) -> String {
        match component_type {
            0b00001 => "1/0モード（シングルモノ）".to_string(),
            0b00010 => "1/0+1/0モード（デュアルモノ）".to_string(),
            0b00011 => "2/0モード(ステレオ)".to_string(),
            0b00100 => "2/1モード".to_string(),
            0b00101 => "3/0モード".to_string(),
            0b00110 => "2/2モード".to_string(),
            0b00111 => "3/1モード".to_string(),
            0b01000 => "3/2モード".to_string(),
            0b01001 => "3/2+LFEモード（3/2.1モード）".to_string(),
            _ => format!("不明なコンポーネントタイプ: 0b{:b}", component_type),
        }
    }

    pub fn get_sampling_rate_name(sampling_rate: u32) -> String {
        match sampling_rate {
            16000 => "16kHz".to_string(),
            22050 => "22.05kHz".to_string(),
            24000 => "24kHz".to_string(),
            32000 => "32kHz".to_string(),
            44100 => "44.1kHz".to_string(),
            48000 => "48kHz".to_string(),
            _ => format!("{}Hz", sampling_rate),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedItem {
    pub r#type: String,
    pub network_id: Option<i32>,
    pub service_id: i32,
    pub event_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramsData(pub Vec<Program>);

impl From<Bytes> for ProgramsData {
    fn from(bytes: Bytes) -> Self {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| ProgramsData(Vec::new()))
    }
}

impl From<ProgramsData> for Bytes {
    fn from(data: ProgramsData) -> Self {
        Bytes::from(serde_json::to_vec(&data).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_program_serialization() {
        let identifiers = ProgramIdentifiers {
            id: 1,
            event_id: 1001,
            service_id: 1,
            network_id: 32736,
        };

        let timing = ProgramTiming {
            start_at: 1619856000000,
            duration: 1800000,
        };

        let program = Program::new(
            identifiers,
            timing,
            true,
            Some("テスト番組".to_string()),
            Some("テスト番組の説明".to_string()),
            vec![Genre { lv1: 7, lv2: 15 }],
            Channel {
                id: 1,
                name: "テストチャンネル".to_string(),
            },
        );

        let json = serde_json::to_string(&program).unwrap();
        let deserialized: Program = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, program.id);
        assert_eq!(deserialized.event_id, program.event_id);
        assert_eq!(deserialized.service_id, program.service_id);
        assert_eq!(deserialized.network_id, program.network_id);
        assert_eq!(deserialized.start_at, program.start_at);
        assert_eq!(deserialized.duration, program.duration);
        assert_eq!(deserialized.end_at, program.start_at + program.duration);
        assert_eq!(deserialized.is_free, program.is_free);
        assert_eq!(deserialized.name, program.name);
        assert_eq!(deserialized.description, program.description);
        assert_eq!(deserialized.genres.len(), program.genres.len());
        assert_eq!(deserialized.genres[0].lv1, program.genres[0].lv1);
        assert_eq!(deserialized.genres[0].lv2, program.genres[0].lv2);
        assert_eq!(deserialized.channel.id, program.channel.id);
        assert_eq!(deserialized.channel.name, program.channel.name);
    }

    #[test]
    fn test_genre_to_string() {
        assert_eq!(
            Genre { lv1: 7, lv2: 0 }.to_string(),
            "アニメ・特撮/国内アニメ"
        );
        assert_eq!(
            Genre { lv1: 7, lv2: 1 }.to_string(),
            "アニメ・特撮/海外アニメ"
        );
        assert_eq!(Genre { lv1: 7, lv2: 2 }.to_string(), "アニメ・特撮/特撮");
        assert_eq!(Genre { lv1: 7, lv2: 15 }.to_string(), "アニメ・特撮/その他");
    }

    #[test]
    fn test_video_component_type_name() {
        assert_eq!(
            Video::get_component_type_name(0xb3),
            "1080i(1125i), アスペクト比16:9 パンベクトルなし"
        );
        assert_eq!(
            Video::get_component_type_name(0xc3),
            "720p(750p), アスペクト比16:9 パンベクトルなし"
        );
    }

    #[test]
    fn test_audio_component_type_name() {
        assert_eq!(
            Audio::get_component_type_name(0b00011),
            "2/0モード(ステレオ)"
        );
    }

    #[test]
    fn test_audio_sampling_rate_name() {
        assert_eq!(Audio::get_sampling_rate_name(48000), "48kHz");
        assert_eq!(Audio::get_sampling_rate_name(44100), "44.1kHz");
    }
}
