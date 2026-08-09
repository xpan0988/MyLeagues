use serde::ser::{Serialize, Serializer};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error(
    "Riot API request failed: service={service} host={host} path={path} status={status} body={body}"
)]
pub struct RiotApiError {
    pub service: &'static str,
    pub host: String,
    pub path: String,
    pub status: u16,
    pub body: String,
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    RiotApi(#[from] RiotApiError),
    #[error("Riot API response could not be parsed: {0}")]
    RiotData(String),
    #[error("Data Dragon error: {0}")]
    StaticData(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("feature is not available yet: {0}")]
    Unavailable(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
