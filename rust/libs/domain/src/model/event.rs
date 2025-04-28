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

        use crate::{model::program::Program, types::Event};

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct Updated {
            pub service_id: i64,
            pub programs: Vec<Program>,
        }
        impl Event for Updated {}
    }
}
