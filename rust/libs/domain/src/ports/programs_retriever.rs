use crate::model::program::Program;

pub trait ProgramsRetriever {
    fn get_programs(&self, service_id: i64) -> Vec<Program>;
}
