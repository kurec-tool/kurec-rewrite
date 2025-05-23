mod sse;
pub use sse::*;
pub mod sse_event;

mod http_client;
pub use http_client::{MirakcApiClient, MirakcApiError};

mod programs_retriever;
pub use programs_retriever::MirakcProgramsRetriever;

#[cfg(test)]
mod tests {
    use tracing_subscriber::{EnvFilter, fmt};

    /// テスト用のロギングを初期化
    pub fn init_test_logging() {
        let _ = fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_test_writer()
            .try_init();
    }
}
