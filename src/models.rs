use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct RequestPayload {
    pub audio_url: String,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct FinalResponse {
    pub transcript: Vec<TranscriptItem>,
    pub topics: Vec<Topic>,
    pub summary: Summary,
    pub action_items: Vec<ActionItem>,
    pub highlights: Vec<Highlight>,
}
