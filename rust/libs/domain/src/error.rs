use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("プログラム取得エラー: {0}")]
    ProgramsRetrievalError(String),

    #[error("サービス(ID={0})が見つかりません")]
    ServiceNotFound(i64),

    #[error("不明なエラー: {0}")]
    UnknownError(String),
}
