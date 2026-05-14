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
    #[error("Google auth error: {0}")]
    Auth(String),
    #[error("Missing environment variable: {0}")]
    Env(String),
    #[error("Timeout error")]
    #[allow(dead_code)]
    Timeout,
    #[error("Audio too long: {0}")]
    #[allow(dead_code)]
    AudioTooLong(String),
}

impl AppError {
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::Timeout => "PROCESSING_TIMEOUT",
            AppError::AudioTooLong(_) => "AUDIO_TOO_LONG",
            AppError::Gemini(_) => "TRANSCRIPTION_FAILED",
            _ => "INTERNAL_ERROR",
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            AppError::Reqwest(e) => format!("Error de conexión: {}", e),
            AppError::Serde(e) => format!("Error de procesamiento interno: {}", e),
            AppError::Io(e) => format!("Error de E/S: {}", e),
            AppError::Gemini(e) => format!("Error al procesar el audio: {}", e),
            AppError::Auth(e) => format!("Error de autenticación: {}", e),
            AppError::Env(e) => format!("Error de configuración: {}", e),
            AppError::Timeout => {
                "La solicitud excedió el tiempo máximo de procesamiento (240 segundos)".to_string()
            }
            AppError::AudioTooLong(msg) => {
                format!("El audio es demasiado largo para procesarlo: {}", msg)
            }
        }
    }
}
