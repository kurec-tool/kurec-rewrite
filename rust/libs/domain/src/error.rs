use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("プログラム保存エラー: {0}")]
    ProgramsStoreError(String),

    #[error("プログラム取得エラー: {0}")]
    ProgramsRetrievalError(String),

    #[error("サービス(ID={0})が見つかりません")]
    ServiceNotFound(i64),

    #[error("画像処理エラー: {0}")]
    ImageProcessingError(String),

    #[error("不明なエラー: {0}")]
    UnknownError(String),
}
