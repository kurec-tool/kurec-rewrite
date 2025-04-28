use crate::error::DomainError;
use crate::model::program::Program;

#[async_trait::async_trait]
pub trait ProgramsRetriever {
    async fn get_programs(&self, service_id: i64) -> Result<Vec<Program>, DomainError>;
}
