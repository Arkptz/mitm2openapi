#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("TNetString parse error at byte {offset}: {message}")]
    TNetParse { offset: usize, message: String },

    #[error("TNetString payload too large: {len} bytes exceeds limit of {max} bytes")]
    TNetStringPayloadTooLarge { len: usize, max: usize },

    #[error("TNetString recursion depth exceeded: depth {depth} exceeds limit of {max}")]
    TNetStringDepthExceeded { depth: usize, max: usize },

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

    #[error("Input too large: {size} bytes exceeds limit of {max} bytes")]
    InputTooLarge { size: u64, max: u64 },

    #[error("Symlink rejected: {}", path.display())]
    SymlinkRejected { path: std::path::PathBuf },

    #[error("Not a regular file: {}", path.display())]
    NotRegularFile { path: std::path::PathBuf },

    #[error("Invalid path parameter identifier: {name:?}")]
    InvalidParamIdent { name: String },

    #[error("Body too large: {size} bytes exceeds limit of {max} bytes")]
    BodyTooLarge { size: usize, max: usize },
}

pub type Result<T> = std::result::Result<T, Error>;
