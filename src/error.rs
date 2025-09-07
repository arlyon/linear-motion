use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Database error: {0}")]
    Database(#[from] fjall::Error),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("GraphQL error: {0}")]
    GraphQL(String),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Motion API error: {message}")]
    MotionApi { message: String },

    #[error("Linear API error: {message}")]
    LinearApi { message: String },

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Authentication failed")]
    Authentication,

    #[error("HTTP header error: {0}")]
    Header(#[from] reqwest::header::InvalidHeaderValue),

    #[error("{0}")]
    Other(String),
}