#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("TNetString parse error at byte {offset}: {message}")]
    TNetParse { offset: usize, message: String },

    #[error("Invalid flow state: {0}")]
    FlowState(String),

    #[error("HAR parse error: {0}")]
    HarParse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(String),

    #[error("Schema error: {0}")]
    Schema(String),
}

pub type Result<T> = std::result::Result<T, Error>;
