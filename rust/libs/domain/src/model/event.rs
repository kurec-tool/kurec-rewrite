pub mod recording {
    pub mod epg {
        use serde::{Deserialize, Serialize};

        use crate::types::Event;

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct EpgUpdated {}
        impl Event for EpgUpdated {}
    }
}
