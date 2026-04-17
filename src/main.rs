mod error;
mod gemini;
mod models;
mod pipeline;

use aws_lambda_events::event::lambda_function_urls::{LambdaFunctionUrlRequest, LambdaFunctionUrlResponse};
use aws_lambda_events::http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent};
use std::sync::Arc;
use tracing::{error, info};

use crate::gemini::GeminiClient;
use crate::models::RequestPayload;
use crate::pipeline::Pipeline;

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .without_time()
        .init();

    info!("Starting AWS Lambda handler (Recording Analyzer)...");

    let func = service_fn(func_handler);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn func_handler(
    event: LambdaEvent<LambdaFunctionUrlRequest>,
) -> Result<LambdaFunctionUrlResponse, LambdaError> {
    // 1. Extraer el body del evento de Function URL
    let body_str = event.payload.body.as_deref().unwrap_or("{}");
    
    // 2. Deserializar el JSON real que envía Flutter
    let payload: RequestPayload = match serde_json::from_str(body_str) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to deserialize request body: {}. Body was: {}", e, body_str);
            return Ok(LambdaFunctionUrlResponse {
                status_code: 400,
                body: Some(format!("Invalid JSON: {}", e).into()),
                headers: HeaderMap::new(),
                is_base64_encoded: false,
                cookies: vec![],
            });
        }
    };

    let audio_url = payload.audio_url;
    info!("Request received for URL: {}", audio_url);

    // Initialize Gemini Client
    let gemini_client = match GeminiClient::new() {
        Ok(client) => Arc::new(client),
        Err(e) => {
            error!("Error initializing Gemini client: {}", e);
            return Ok(LambdaFunctionUrlResponse {
                status_code: 500,
                body: Some("Error initializing Gemini client".into()),
                headers: HeaderMap::new(),
                is_base64_encoded: false,
                cookies: vec![],
            });
        }
    };

    let pipeline = Pipeline::new(gemini_client);

    // 3. Ejecutar el pipeline
    match pipeline.run_pipeline(&audio_url).await {
        Ok(response) => {
            info!("Audio processing successful.");
            let json_response = serde_json::to_string(&response)?;
            
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

            Ok(LambdaFunctionUrlResponse {
                status_code: 200,
                body: Some(json_response.into()),
                headers,
                is_base64_encoded: false,
                cookies: vec![],
            })
        }
        Err(e) => {
            error!("Pipeline execution failed: {}", e);
            Ok(LambdaFunctionUrlResponse {
                status_code: 500,
                body: Some(format!("Pipeline error: {}", e).into()),
                headers: HeaderMap::new(),
                is_base64_encoded: false,
                cookies: vec![],
            });
        }
    }
}
