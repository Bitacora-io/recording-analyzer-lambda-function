use std::sync::Arc;
use futures::try_join;
use tracing::info;

use crate::models::*;
use crate::error::AppError;
use crate::gemini::GeminiClient;

pub struct Pipeline {
    gemini: Arc<GeminiClient>,
}

impl Pipeline {
    pub fn new(gemini: Arc<GeminiClient>) -> Self {
        Self { gemini }
    }

    /// Executes the entire audio analysis pipeline
    pub async fn run_pipeline(&self, audio_url: &str) -> Result<FinalResponse, AppError> {
        info!("Starting pipeline for URL: {}", audio_url);
        
        // Step 1: Transcription (this step is blocking for the others)
        info!("Stage 1: Obtaining transcription...");
        let transcript = self.transcribe(audio_url).await?;

        // Convert the transcription to a JSON string to pass it as input to the next steps
        let transcript_json = serde_json::to_string(&transcript)?;

        info!("Stage 2-5: Extracting information in parallel...");
        // Steps 2 to 5: Executed concurrently as they are independent
        let (topics, summary, action_items, highlights) = try_join!(
            self.extract_topics(&transcript_json),
            self.extract_summary(&transcript_json),
            self.extract_action_items(&transcript_json),
            self.extract_highlights(&transcript_json)
        )?;

        info!("Pipeline completed successfully.");
        Ok(FinalResponse {
            transcript,
            topics,
            summary,
            action_items,
            highlights,
        })
    }

    async fn transcribe(&self, audio_url: &str) -> Result<Vec<TranscriptItem>, AppError> {
        let prompt = "You are a specialized audio transcription system. \
            Analyze the provided audio file URL and generate a full transcript in its original language (e.g., if the audio is in Spanish, transcribe in Spanish; if in English, transcribe in English). \
            Provide timestamps and speaker diarization (e.g., 'Speaker A', 'Speaker B'). \
            Output strictly as a JSON array of objects, with each object having exactly these keys: \
            'start_time' (string, e.g., '00:00:00'), 'end_time' (string), 'speaker' (string), 'text' (string). \
            Do not include any other text, markdown blocks, or explanation. Ensure valid JSON format.";

        let result = self.gemini.call_gemini(prompt, audio_url).await?;
        let parsed: Vec<TranscriptItem> = serde_json::from_str(&result)?;
        Ok(parsed)
    }

    async fn extract_topics(&self, transcript: &str) -> Result<Vec<Topic>, AppError> {
        let prompt = "Analyze the provided meeting transcript and perform topic segmentation. \
            The output (title and description) MUST be in the same language as the transcript (e.g., if the transcript is in Spanish, the response must be in Spanish). \
            Group the discussion into distinct topics based on chronological time. \
            Output strictly as a JSON array of objects with exactly these keys: \
            'start_time' (string), 'end_time' (string), 'title' (string, short summary of topic), \
            'description' (string, detailed explanation). \
            Do not include any other text or markdown formatting. Ensure valid JSON format.";

        let result = self.gemini.call_gemini(prompt, transcript).await?;
        let parsed: Vec<Topic> = serde_json::from_str(&result)?;
        Ok(parsed)
    }

    async fn extract_summary(&self, transcript: &str) -> Result<Summary, AppError> {
        let prompt = "Analyze the provided meeting transcript and generate an executive summary. \
            The summary MUST be in the same language as the transcript (e.g., if the transcript is in Spanish, the response must be in Spanish). \
            The summary should capture the main goals, decisions, and overall outcomes. \
            Output strictly as a JSON object with a single key 'executive_summary' containing an array of strings (bullet points). \
            Do not include any other text or markdown formatting. Ensure valid JSON format.";

        let result = self.gemini.call_gemini(prompt, transcript).await?;
        let parsed: Summary = serde_json::from_str(&result)?;
        Ok(parsed)
    }

    async fn extract_action_items(&self, transcript: &str) -> Result<Vec<ActionItem>, AppError> {
        let prompt = "Analyze the provided meeting transcript and extract all action items and tasks. \
            The output (task, owner, deadline, priority) MUST be in the same language as the transcript where applicable, but maintaining the JSON structure. \
            For 'priority', use equivalent terms (e.g., 'Alta', 'Media', 'Baja' for Spanish). \
            Output strictly as a JSON array of objects with exactly these keys: \
            'task' (string), 'owner' (string or null if not explicitly identified), \
            'deadline' (string or null if not identified), 'priority' (string, or null). \
            Do not include any other text or markdown formatting. Ensure valid JSON format.";

        let result = self.gemini.call_gemini(prompt, transcript).await?;
        let parsed: Vec<ActionItem> = serde_json::from_str(&result)?;
        Ok(parsed)
    }

    async fn extract_highlights(&self, transcript: &str) -> Result<Vec<Highlight>, AppError> {
        let prompt = "Analyze the provided meeting transcript and extract key highlights or memorable quotes. \
            The output (description and reason) MUST be in the same language as the transcript (e.g., if the transcript is in Spanish, the response must be in Spanish). \
            Output strictly as a JSON array of objects with exactly these keys: \
            'start_time' (string), 'end_time' (string), 'description' (string, the highlight or quote), \
            'reason' (string, why this is a highlight). \
            Do not include any other text or markdown formatting. Ensure valid JSON format.";

        let result = self.gemini.call_gemini(prompt, transcript).await?;
        let parsed: Vec<Highlight> = serde_json::from_str(&result)?;
        Ok(parsed)
    }
}
