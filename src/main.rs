mod error;
mod gemini;
mod models;
mod pipeline;

use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent};
use std::sync::Arc;
use tracing::{error, info};

use crate::gemini::GeminiClient;
use crate::models::{FinalResponse, RequestPayload};
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

async fn func_handler(event: LambdaEvent<RequestPayload>) -> Result<FinalResponse, LambdaError> {
    let payload = event.payload;
    let audio_url = payload.audio_url;

    info!("Request received for URL: {}", audio_url);

    // Initialize Gemini Client
    let gemini_client = match GeminiClient::new() {
        Ok(client) => Arc::new(client),
        Err(e) => {
            error!("Error initializing Gemini client: {}", e);
            return Err(e.into());
        }
    };

    let pipeline = Pipeline::new(gemini_client);

    // Execute the pipeline
    match pipeline.run_pipeline(&audio_url).await {
        Ok(response) => {
            info!("Audio processing successful.");
            Ok(response)
        }
        Err(e) => {
            error!("Pipeline execution failed: {}", e);
            Err(e.into())
        }
    }
}
