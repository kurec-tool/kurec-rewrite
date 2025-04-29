pub mod recording {
    pub mod epg {
        use serde::{Deserialize, Serialize};

        use crate::types::Event;

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct Updated {
            pub service_id: i64,
        }
        impl Event for Updated {}
    }
    pub mod programs {
        use serde::{Deserialize, Serialize};

        use crate::types::Event;

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct Updated {
            pub service_id: i64,
            pub mirakc_url: String,
        }
        impl Event for Updated {}
    }
}
