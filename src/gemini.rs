use std::env;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use percent_encoding::percent_decode_str;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::error::AppError;

const MAX_RETRIES: u32 = 3;
const MODEL_NAME: &str = "gemini-3-flash-preview";
const DEFAULT_CREDENTIALS_PATH: &str = "bitacora-657e2-e7b400931917.json";
const GOOGLE_OAUTH_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const TOKEN_REFRESH_SKEW_SECONDS: u64 = 60;

pub struct GeminiClient {
    client: Client,
    credentials: ServiceAccountCredentials,
    project_id: String,
    location: String,
    model: String,
    token_cache: Mutex<Option<CachedToken>>,
}

#[derive(Clone, Deserialize)]
struct ServiceAccountCredentials {
    project_id: String,
    private_key_id: String,
    private_key: String,
    client_email: String,
    token_uri: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Serialize)]
struct JwtClaims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

struct CachedToken {
    value: String,
    expires_at: u64,
}

impl GeminiClient {
    pub fn new() -> Result<Self, AppError> {
        let credentials = load_service_account_credentials()?;
        let project_id =
            env::var("VERTEX_AI_PROJECT_ID").unwrap_or_else(|_| credentials.project_id.clone());
        let location = env::var("VERTEX_AI_LOCATION").unwrap_or_else(|_| "global".to_string());
        let model = env::var("VERTEX_AI_MODEL").unwrap_or_else(|_| MODEL_NAME.to_string());

        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // Increased timeout for large audio files
            .build()?;

        Ok(Self {
            client,
            credentials,
            project_id,
            location,
            model,
            token_cache: Mutex::new(None),
        })
    }

    /// Asynchronous call to the Gemini API with retry logic
    pub async fn call_gemini(&self, prompt: &str, input_text: &str) -> Result<String, AppError> {
        let full_prompt = format!("{}\n\nInput data:\n{}", prompt, input_text);

        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [{"text": full_prompt}]
            }],
            "generationConfig": {
                "responseMimeType": "application/json"
            }
        });

        self.execute_request(body).await
    }

    /// Call Gemini on Vertex AI using an audio URL.
    ///
    /// Firebase Storage URLs are converted to gs:// URIs so Vertex AI can read
    /// the object directly. Other URLs are passed through as fileData URIs.
    pub async fn call_gemini_with_audio_url(
        &self,
        prompt: &str,
        audio_url: &str,
        mime_type: &str,
    ) -> Result<String, AppError> {
        let file_uri = if let Some(gcs_uri) = firebase_storage_url_to_gcs_uri(audio_url) {
            tracing::info!("Using Vertex AI fileData from {}", gcs_uri);
            gcs_uri
        } else {
            tracing::info!("Using Vertex AI fileData from source URL");
            audio_url.to_string()
        };

        let body = json!({
            "contents": [{
                "role": "user",
                "parts": [
                    {"text": prompt},
                    {
                        "fileData": {
                            "mimeType": mime_type,
                            "fileUri": file_uri
                        }
                    }
                ]
            }],
            "generationConfig": {
                "responseMimeType": "application/json"
            }
        });

        self.execute_request(body).await
    }

    async fn execute_request(&self, body: Value) -> Result<String, AppError> {
        let url = self.generate_content_url();
        let mut attempt = 0;
        loop {
            attempt += 1;
            let access_token = self.access_token().await?;

            match self
                .client
                .post(&url)
                .bearer_auth(access_token)
                .json(&body)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        let json_resp: Value = response.json().await?;
                        return if let Some(text) =
                            json_resp["candidates"][0]["content"]["parts"][0]["text"].as_str()
                        {
                            Ok(text.to_string())
                        } else {
                            Err(AppError::Gemini(
                                "Invalid or unexpected response format".into(),
                            ))
                        };
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();

                        if attempt >= MAX_RETRIES {
                            return Err(AppError::Gemini(format!(
                                "HTTP Error {}: {}",
                                status, error_text
                            )));
                        }
                        tracing::warn!(
                            "Retry {}/{} after HTTP error {}",
                            attempt,
                            MAX_RETRIES,
                            status
                        );
                    }
                }
                Err(e) => {
                    if attempt >= MAX_RETRIES {
                        return Err(e.into());
                    }
                    tracing::warn!(
                        "Retry {}/{} after connection error: {}",
                        attempt,
                        MAX_RETRIES,
                        e
                    );
                }
            }

            sleep(Duration::from_secs(2u64.pow(attempt))).await;
        }
    }

    fn generate_content_url(&self) -> String {
        let host = if self.location == "global" {
            "aiplatform.googleapis.com".to_string()
        } else {
            format!("{}-aiplatform.googleapis.com", self.location)
        };

        format!(
            "https://{}/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
            host, self.project_id, self.location, self.model
        )
    }

    async fn access_token(&self) -> Result<String, AppError> {
        let now = unix_timestamp()?;
        if let Some(token) = self.cached_token(now)? {
            return Ok(token);
        }

        let token = self.fetch_access_token(now).await?;
        let mut cache = self
            .token_cache
            .lock()
            .map_err(|_| AppError::Auth("Token cache lock poisoned".into()))?;
        cache.replace(CachedToken {
            value: token.access_token.clone(),
            expires_at: now + token.expires_in,
        });

        Ok(token.access_token)
    }

    fn cached_token(&self, now: u64) -> Result<Option<String>, AppError> {
        let cache = self
            .token_cache
            .lock()
            .map_err(|_| AppError::Auth("Token cache lock poisoned".into()))?;

        Ok(cache.as_ref().and_then(|token| {
            let valid_until = now + TOKEN_REFRESH_SKEW_SECONDS;
            (token.expires_at > valid_until).then(|| token.value.clone())
        }))
    }

    async fn fetch_access_token(&self, now: u64) -> Result<TokenResponse, AppError> {
        let assertion = self.jwt_assertion(now)?;
        let response = self
            .client
            .post(&self.credentials.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", assertion.as_str()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!(
                "Failed to fetch Google access token: HTTP {}: {}",
                status, error_text
            )));
        }

        Ok(response.json().await?)
    }

    fn jwt_assertion(&self, now: u64) -> Result<String, AppError> {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.credentials.private_key_id.clone());

        let claims = JwtClaims {
            iss: &self.credentials.client_email,
            scope: GOOGLE_OAUTH_SCOPE,
            aud: &self.credentials.token_uri,
            iat: now,
            exp: now + 3600,
        };

        let key = EncodingKey::from_rsa_pem(self.credentials.private_key.as_bytes())
            .map_err(|e| AppError::Auth(format!("Invalid service account private key: {}", e)))?;

        encode(&header, &claims, &key)
            .map_err(|e| AppError::Auth(format!("Failed to sign service account JWT: {}", e)))
    }
}

fn load_service_account_credentials() -> Result<ServiceAccountCredentials, AppError> {
    if let Ok(json) = env::var("GOOGLE_SERVICE_ACCOUNT_JSON") {
        return Ok(serde_json::from_str(&json)?);
    }

    let path = env::var("GOOGLE_APPLICATION_CREDENTIALS")
        .unwrap_or_else(|_| DEFAULT_CREDENTIALS_PATH.to_string());
    let json = std::fs::read_to_string(&path).map_err(|e| {
        AppError::Env(format!(
            "Set GOOGLE_SERVICE_ACCOUNT_JSON or GOOGLE_APPLICATION_CREDENTIALS. Could not read {}: {}",
            path, e
        ))
    })?;

    Ok(serde_json::from_str(&json)?)
}

fn unix_timestamp() -> Result<u64, AppError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|e| AppError::Auth(format!("System clock is before UNIX epoch: {}", e)))
}

fn firebase_storage_url_to_gcs_uri(url: &str) -> Option<String> {
    let marker = "firebasestorage.googleapis.com/v0/b/";
    let after_marker = url.split_once(marker)?.1;
    let (bucket, after_bucket) = after_marker.split_once("/o/")?;
    let encoded_object = after_bucket.split(['?', '#']).next()?;
    let object = percent_decode_str(encoded_object).decode_utf8().ok()?;

    (!bucket.is_empty() && !object.is_empty()).then(|| format!("gs://{}/{}", bucket, object))
}
