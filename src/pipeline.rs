use std::sync::Arc;
use futures::try_join;
use tracing::info;
use serde::Deserialize;

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

        info!("Stage 2-6: Extracting information in parallel...");
        // Steps 2 to 6: Executed concurrently as they are independent
        let (topics, summary, action_items, highlights, title) = try_join!(
            self.extract_topics(&transcript_json),
            self.extract_summary(&transcript_json),
            self.extract_action_items(&transcript_json),
            self.extract_highlights(&transcript_json),
            self.extract_title(&transcript_json)
        )?;

        info!("Calculating speaker participation...");
        let participation = self.calculate_participation(&transcript);

        info!("Pipeline completed successfully.");
        Ok(FinalResponse {
            title,
            transcript,
            topics,
            summary,
            action_items,
            highlights,
            participation,
        })
    }

    async fn transcribe(&self, audio_url: &str) -> Result<Vec<TranscriptItem>, AppError> {
        let prompt = "You are a specialized audio transcription system. \
            Analyze the provided audio and generate a full transcript in its original language. \
            Provide timestamps and speaker diarization (e.g., 'Speaker A', 'Speaker B'). \
            To ensure the full transcript fits in the response, group consecutive sentences from the same speaker into larger blocks (aim for 30-60 seconds per block when possible). \
            Output strictly as a JSON array of objects, with each object having exactly these keys: \
            'start_time' (string, e.g., '00:00:00'), 'end_time' (string), 'speaker' (string), 'text' (string). \
            Do not include any other text, markdown blocks, or explanation. Ensure valid JSON format.";

        // 1. Download file to /tmp
        let temp_path = "/tmp/recording_to_process";
        self.gemini.download_file(audio_url, temp_path).await?;

        // 2. Determine mime type (simple extension-based)
        let mime_type = if audio_url.ends_with(".m4a") {
            "audio/mp4"
        } else if audio_url.ends_with(".wav") {
            "audio/wav"
        } else {
            "audio/mpeg" // Default to mp3
        };

        // 3. Upload to Gemini File API
        let file_uri = self.gemini.upload_file(temp_path, mime_type).await?;

        // 4. Call Gemini with File
        let result = self.gemini.call_gemini_with_file(prompt, &file_uri, mime_type).await?;

        // 5. Cleanup temp file
        let _ = tokio::fs::remove_file(temp_path).await;

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

    async fn extract_title(&self, transcript: &str) -> Result<String, AppError> {
        let prompt = "Analyze the provided meeting transcript and generate a concise and descriptive title for the session. \
            The title MUST be in the same language as the transcript. \
            Output strictly as a JSON object with a single key 'title' containing the string value. \
            Do not include any other text or markdown formatting. Ensure valid JSON format.";

        let result = self.gemini.call_gemini(prompt, transcript).await?;
        
        #[derive(Deserialize)]
        struct TitleResponse { title: String }
        let parsed: TitleResponse = serde_json::from_str(&result)?;
        Ok(parsed.title)
    }

    fn calculate_participation(&self, transcript: &[TranscriptItem]) -> Vec<SpeakerParticipation> {
        use std::collections::HashMap;

        let mut durations: HashMap<String, f32> = HashMap::new();
        let mut total_duration: f32 = 0.0;

        for item in transcript {
            let start = parse_timestamp(&item.start_time);
            let end = parse_timestamp(&item.end_time);
            let duration = (end - start).max(0.0);

            *durations.entry(item.speaker.clone()).or_insert(0.0) += duration;
            total_duration += duration;
        }

        let mut result = Vec::new();
        if total_duration > 0.0 {
            for (speaker, duration) in durations {
                result.push(SpeakerParticipation {
                    speaker,
                    duration_seconds: duration,
                    percentage: (duration / total_duration) * 100.0,
                });
            }
        }
        
        // Sort by percentage descending
        result.sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap_or(std::cmp::Ordering::Equal));
        result
    }
}

fn parse_timestamp(ts: &str) -> f32 {
    let parts: Vec<&str> = ts.split(':').collect();
    let mut seconds = 0.0;
    if parts.len() == 3 {
        // HH:MM:SS
        seconds += parts[0].parse::<f32>().unwrap_or(0.0) * 3600.0;
        seconds += parts[1].parse::<f32>().unwrap_or(0.0) * 60.0;
        seconds += parts[2].parse::<f32>().unwrap_or(0.0);
    } else if parts.len() == 2 {
        // MM:SS
        seconds += parts[0].parse::<f32>().unwrap_or(0.0) * 60.0;
        seconds += parts[1].parse::<f32>().unwrap_or(0.0);
    } else {
        seconds += ts.parse::<f32>().unwrap_or(0.0);
    }
    seconds
}
