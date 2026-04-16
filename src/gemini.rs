use std::env;
use reqwest::Client;
use std::time::Duration;
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::error::AppError;

const MAX_RETRIES: u32 = 3;

pub struct GeminiClient {
    client: Client,
    api_key: String,
}

impl GeminiClient {
    pub fn new() -> Result<Self, AppError> {
        let api_key = env::var("GEMINI_API_KEY")
            .map_err(|_| AppError::Env("GEMINI_API_KEY must be set in environment variables".into()))?;
        
        let client = Client::builder()
            .timeout(Duration::from_secs(90)) // Lambda processing can take time
            .build()?;
            
        Ok(Self { client, api_key })
    }

    /// Asynchronous call to the Gemini API with retry logic (retry backoff)
    /// Returns a valid JSON as a String
    pub async fn call_gemini(&self, prompt: &str, input: &str) -> Result<String, AppError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent?key={}",
            self.api_key
        );

        let full_prompt = format!("{}\n\nInput data:\n{}", prompt, input);

        let body = json!({
            "contents": [{
                "parts": [{"text": full_prompt}]
            }],
            "generationConfig": {
                // Force Gemini to return content in JSON format (Gemini 1.5 JSON mode)
                "response_mime_type": "application/json"
            }
        });

        let mut attempt = 0;
        loop {
            attempt += 1;
            
            match self.client.post(&url).json(&body).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        let json_resp: Value = response.json().await?;
                        return if let Some(text) = json_resp["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                            Ok(text.to_string())
                        } else {
                            Err(AppError::Gemini("Invalid or unexpected response format".into()))
                        }
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        
                        if attempt >= MAX_RETRIES {
                            return Err(AppError::Gemini(format!("HTTP Error {}: {}", status, error_text)));
                        }
                        tracing::warn!("Retry {}/{} after HTTP error {}", attempt, MAX_RETRIES, status);
                    }
                }
                Err(e) => {
                    if attempt >= MAX_RETRIES {
                        return Err(e.into());
                    }
                    tracing::warn!("Retry {}/{} after connection error: {}", attempt, MAX_RETRIES, e);
                }
            }
            
            // Exponential backoff: wait 2^attempt seconds (2, 4, 8)
            sleep(Duration::from_secs(2u64.pow(attempt))).await;
        }
    }
}
