mod error;
mod gemini;
mod models;
mod pipeline;

use aws_lambda_events::event::lambda_function_urls::{
    LambdaFunctionUrlRequest, LambdaFunctionUrlResponse,
};
use aws_lambda_events::http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent};
use serde::Serialize;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{error, info};

use crate::gemini::GeminiClient;
use crate::models::{ErrorResponse, RequestPayload};
use crate::pipeline::Pipeline;

const PROCESSING_TIMEOUT_SECONDS: u64 = 240;

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

fn json_response<T: Serialize>(status_code: i64, body: &T) -> Result<LambdaFunctionUrlResponse, LambdaError> {
    let json = serde_json::to_string(body)?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(LambdaFunctionUrlResponse {
        status_code,
        body: Some(json),
        headers,
        is_base64_encoded: false,
        cookies: vec![],
    })
}

fn error_response(status_code: i64, code: &str, message: &str) -> Result<LambdaFunctionUrlResponse, LambdaError> {
    json_response(status_code, &ErrorResponse::new(code, message))
}

async fn func_handler(
    event: LambdaEvent<LambdaFunctionUrlRequest>,
) -> Result<LambdaFunctionUrlResponse, LambdaError> {
    let start = Instant::now();
    let body_str = event.payload.body.as_deref().unwrap_or("{}");

    let payload: RequestPayload = match serde_json::from_str(body_str) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to deserialize request body: {}. Body was: {}", e, body_str);
            return error_response(400, "INVALID_REQUEST", &format!("JSON inválido: {}", e));
        }
    };

    let audio_url = payload.audio_url;
    info!("Request received for URL: {}", audio_url);

    let gemini_client = match GeminiClient::new() {
        Ok(client) => Arc::new(client),
        Err(e) => {
            error!("Error initializing Gemini client: {}", e);
            return error_response(500, "INTERNAL_ERROR", "Error al inicializar el cliente de Gemini");
        }
    };

    let pipeline = Pipeline::new(gemini_client);

    let timeout_result = tokio::time::timeout(
        std::time::Duration::from_secs(PROCESSING_TIMEOUT_SECONDS),
        pipeline.run_pipeline(&audio_url),
    )
    .await;

    let response = match timeout_result {
        Ok(Ok(resp)) => json_response(200, &resp)?,
        Ok(Err(e)) => {
            error!("Pipeline execution failed: {}", e);
            let err_resp = ErrorResponse::from_app_error(&e);
            json_response(500, &err_resp)?
        }
        Err(_) => {
            error!("Pipeline timed out after {}s", PROCESSING_TIMEOUT_SECONDS);
            error_response(408, "PROCESSING_TIMEOUT", "La solicitud excedió el tiempo máximo de procesamiento (240 segundos)")?
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    let truncated_url: String = audio_url.chars().take(120).collect();
    let unix_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    info!(
        received_at = unix_now,
        audio_url = %truncated_url,
        processing_duration_ms = duration_ms,
        status_code = response.status_code,
        "Request completed"
    );

    Ok(response)
}
