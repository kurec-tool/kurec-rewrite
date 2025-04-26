mod toxiproxy;
pub use toxiproxy::*;
use tracing_subscriber::{EnvFilter, fmt};

pub fn init_test_logging() {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
}
