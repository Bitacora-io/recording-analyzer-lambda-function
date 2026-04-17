use std::env;
use reqwest::Client;
use std::time::Duration;
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::error::AppError;

const MAX_RETRIES: u32 = 3;
const MODEL_NAME: &str = "gemini-3-flash-preview";

pub struct GeminiClient {
    client: Client,
    api_key: String,
}

impl GeminiClient {
    pub fn new() -> Result<Self, AppError> {
        let api_key = env::var("GEMINI_API_KEY")
            .map_err(|_| AppError::Env("GEMINI_API_KEY must be set in environment variables".into()))?;
        
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // Increased timeout for large audio files
            .build()?;
            
        Ok(Self { client, api_key })
    }

    /// Download a file from a URL to a local path
    pub async fn download_file(&self, url: &str, dest_path: &str) -> Result<(), AppError> {
        tracing::info!("Downloading file from {} to {}", url, dest_path);
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(AppError::Gemini(format!("Failed to download file: HTTP {}", response.status())));
        }
        let content = response.bytes().await?;
        tokio::fs::write(dest_path, content).await?;
        Ok(())
    }

    /// Upload a file to Gemini File API
    pub async fn upload_file(&self, file_path: &str, mime_type: &str) -> Result<String, AppError> {
        tracing::info!("Uploading file {} to Gemini File API", file_path);
        
        let metadata = json!({
            "file": {
                "display_name": "audio_recording"
            }
        });

        let file_size = tokio::fs::metadata(file_path).await?.len();

        // 1. Initiate resumable upload
        let url = format!(
            "https://generativelanguage.googleapis.com/upload/v1beta/files?key={}",
            self.api_key
        );

        let response = self.client.post(&url)
            .header("X-Goog-Upload-Protocol", "resumable")
            .header("X-Goog-Upload-Command", "start")
            .header("X-Goog-Upload-Header-Content-Length", file_size.to_string())
            .header("X-Goog-Upload-Header-Content-Type", mime_type)
            .json(&metadata)
            .send()
            .await?;

        if !response.status().is_success() {
            let err = response.text().await?;
            return Err(AppError::Gemini(format!("Failed to initiate upload: {}", err)));
        }

        let upload_url = response.headers()
            .get("x-goog-upload-url")
            .ok_or_else(|| AppError::Gemini("No upload URL received".into()))?
            .to_str()
            .map_err(|_| AppError::Gemini("Invalid upload URL".into()))?;

        // 2. Upload the actual file content
        let file_content = tokio::fs::read(file_path).await?;
        let response = self.client.put(upload_url)
            .header("X-Goog-Upload-Offset", "0")
            .header("X-Goog-Upload-Command", "upload, finalize")
            .body(file_content)
            .send()
            .await?;

        if !response.status().is_success() {
            let err = response.text().await?;
            return Err(AppError::Gemini(format!("Failed to upload file content: {}", err)));
        }

        let json_resp: Value = response.json().await?;
        let file_uri = json_resp["file"]["uri"].as_str()
            .ok_or_else(|| AppError::Gemini("No file URI in response".into()))?
            .to_string();
        let file_name = json_resp["file"]["name"].as_str()
            .ok_or_else(|| AppError::Gemini("No file name in response".into()))?
            .to_string();

        // 3. Wait for the file to be ACTIVE
        self.wait_for_file_active(&file_name).await?;

        Ok(file_uri)
    }

    async fn wait_for_file_active(&self, file_name: &str) -> Result<(), AppError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/{}?key={}",
            file_name, self.api_key
        );

        for _ in 0..20 { // Wait up to 100 seconds
            let response = self.client.get(&url).send().await?;
            let json_resp: Value = response.json().await?;
            
            let state = json_resp["state"].as_str().unwrap_or("PROCESSING");
            tracing::info!("File state: {}", state);
            
            if state == "ACTIVE" {
                return Ok(());
            } else if state == "FAILED" {
                return Err(AppError::Gemini("File processing failed".into()));
            }
            
            sleep(Duration::from_secs(5)).await;
        }

        Err(AppError::Gemini("Timeout waiting for file to be active".into()))
    }

    /// Asynchronous call to the Gemini API with retry logic
    pub async fn call_gemini(&self, prompt: &str, input_text: &str) -> Result<String, AppError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            MODEL_NAME, self.api_key
        );

        let full_prompt = format!("{}\n\nInput data:\n{}", prompt, input_text);

        let body = json!({
            "contents": [{
                "parts": [{"text": full_prompt}]
            }],
            "generationConfig": {
                "response_mime_type": "application/json"
            }
        });

        self.execute_request(&url, body).await
    }

    /// Call Gemini using a file previously uploaded to the File API
    pub async fn call_gemini_with_file(&self, prompt: &str, file_uri: &str, mime_type: &str) -> Result<String, AppError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            MODEL_NAME, self.api_key
        );

        let body = json!({
            "contents": [{
                "parts": [
                    {"text": prompt},
                    {
                        "file_data": {
                            "mime_type": mime_type,
                            "file_uri": file_uri
                        }
                    }
                ]
            }],
            "generationConfig": {
                "response_mime_type": "application/json"
            }
        });

        self.execute_request(&url, body).await
    }

    async fn execute_request(&self, url: &str, body: Value) -> Result<String, AppError> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            
            match self.client.post(url).json(&body).send().await {
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
            
            sleep(Duration::from_secs(2u64.pow(attempt))).await;
        }
    }
}
