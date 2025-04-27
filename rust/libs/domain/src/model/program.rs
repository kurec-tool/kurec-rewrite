use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub id: i64,
    pub event_id: i32,
    pub service_id: i32,
    pub network_id: i32,
    pub start_at: i64,
    pub duration: i64,
    pub is_free: bool,
    pub name: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<Genre>,
    pub channel: Channel,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_program_serialization() {
        let program = Program {
            id: 1,
            event_id: 1001,
            service_id: 1,
            network_id: 32736,
            start_at: 1619856000000,
            duration: 1800000,
            is_free: true,
            name: Some("テスト番組".to_string()),
            description: Some("テスト番組の説明".to_string()),
            genres: vec![
                Genre {
                    lv1: 7,
                    lv2: 15,
                },
            ],
            channel: Channel {
                id: 1,
                name: "テストチャンネル".to_string(),
            },
        };

        let json = serde_json::to_string(&program).unwrap();
        let deserialized: Program = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, program.id);
        assert_eq!(deserialized.event_id, program.event_id);
        assert_eq!(deserialized.service_id, program.service_id);
        assert_eq!(deserialized.network_id, program.network_id);
        assert_eq!(deserialized.start_at, program.start_at);
        assert_eq!(deserialized.duration, program.duration);
        assert_eq!(deserialized.is_free, program.is_free);
        assert_eq!(deserialized.name, program.name);
        assert_eq!(deserialized.description, program.description);
        assert_eq!(deserialized.genres.len(), program.genres.len());
        assert_eq!(deserialized.genres[0].lv1, program.genres[0].lv1);
        assert_eq!(deserialized.genres[0].lv2, program.genres[0].lv2);
        assert_eq!(deserialized.channel.id, program.channel.id);
        assert_eq!(deserialized.channel.name, program.channel.name);
    }
}
