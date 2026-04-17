use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("API request failed: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("JSON serialization/deserialization failed: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Gemini API error: {0}")]
    Gemini(String),
    #[error("Missing environment variable: {0}")]
    Env(String),
    #[error("Timeout error")]
    #[allow(dead_code)]
    Timeout,
}
