use serde::{Deserialize, Serialize};
use crate::error::AppError;

#[derive(Deserialize, Debug)]
pub struct RequestPayload {
    pub audio_url: String,
}

#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub error: bool,
    pub code: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: true,
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn from_app_error(err: &AppError) -> Self {
        Self {
            error: true,
            code: err.error_code().to_string(),
            message: err.user_message(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TranscriptItem {
    pub start_time: String,
    pub end_time: String,
    pub speaker: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Topic {
    pub start_time: String,
    pub end_time: String,
    pub title: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Summary {
    pub executive_summary: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionItem {
    pub task: String,
    pub owner: Option<String>,
    pub deadline: Option<String>,
    pub priority: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Highlight {
    pub start_time: String,
    pub end_time: String,
    pub description: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SpeakerParticipation {
    pub speaker: String,
    pub percentage: f32,
    pub duration_seconds: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FinalResponse {
    pub title: String,
    pub transcript: Vec<TranscriptItem>,
    pub topics: Vec<Topic>,
    pub summary: Summary,
    pub action_items: Vec<ActionItem>,
    pub highlights: Vec<Highlight>,
    pub participation: Vec<SpeakerParticipation>,
}
