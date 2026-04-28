#!/bin/bash

# Exit immediately if a command fails
set -e

echo "🔍 Preparing local test environment..."

# 1. Load Vertex AI configuration from .env file if it exists
if [ -f .env ]; then
    set -a
    # shellcheck disable=SC1091
    source .env
    set +a
    echo "✅ Environment variables loaded from .env"
else
    echo "⚠️ .env file not found. Using system environment variables."
fi

# Validate that Google credentials are configured
if [ -n "$GOOGLE_SERVICE_ACCOUNT" ] && [ -z "$GOOGLE_SERVICE_ACCOUNT_JSON" ]; then
    export GOOGLE_SERVICE_ACCOUNT_JSON="$GOOGLE_SERVICE_ACCOUNT"
fi

if { [ -z "$GOOGLE_SERVICE_ACCOUNT_JSON" ] || [ "$GOOGLE_SERVICE_ACCOUNT_JSON" == "" ]; } && \
   { [ -z "$GOOGLE_APPLICATION_CREDENTIALS" ] || [ "$GOOGLE_APPLICATION_CREDENTIALS" == "" ]; }; then
    echo "❌ Error: Google service account credentials are not configured."
    echo "Please create a .env file by copying .env.example and configure either:"
    echo "  GOOGLE_SERVICE_ACCOUNT_JSON"
    echo "  GOOGLE_APPLICATION_CREDENTIALS"
    echo "  cp .env.example .env"
    exit 1
fi

# 2. Start Lambda emulator in the background
echo "🚀 Compiling and starting local emulator (cargo lambda watch)..."
# We use a temporary file to capture errors if the server fails to start
cargo lambda watch > lambda_watch.log 2>&1 &
WATCH_PID=$!

# Configure a "trap" to ensure we stop the Lambda process when exiting the script,
# regardless of whether the test succeeds or fails.
# shellcheck disable=SC2064
trap "echo '🛑 Stopping local emulator...'; kill $WATCH_PID 2>/dev/null; rm -f lambda_watch.log" EXIT

# 3. Wait for the server to be ready (Rust compilation may take a few seconds)
echo "⏳ Waiting 5 seconds for compilation to finish and the server to be ready..."
sleep 5

# Check that the background process is still alive
if ! kill -0 $WATCH_PID 2>/dev/null; then
    echo "❌ Error: Local server failed to start. Check the logs:"
    cat lambda_watch.log
    exit 1
fi

# 4. Run test invocation
echo "📡 Sending test event..."

# You can change this URL for a real publicly accessible audio
TEST_AUDIO_URL="https://firebasestorage.googleapis.com/v0/b/bitacora-657e2.appspot.com/o/vertex_temp%2F36241591-600b-40e3-8df2-accd6048ca94_1776617411856_audio_recording_1776617401389.m4a?alt=media&token=9b767b78-d55e-47b3-a65b-b869f06803b4"

# Create the nested JSON structure required by LambdaFunctionUrlRequest
RAW_PAYLOAD="{ \"audio_url\": \"$TEST_AUDIO_URL\" }"
WRAPPED_PAYLOAD=$(jq -n --arg body "$RAW_PAYLOAD" '{
  "version": "2.0",
  "routeKey": "$default",
  "rawPath": "/",
  "rawQueryString": "",
  "headers": {
    "content-type": "application/json"
  },
  "requestContext": {
    "http": {
      "method": "POST",
      "path": "/",
      "protocol": "HTTP/1.1",
      "sourceIp": "127.0.0.1",
      "userAgent": "Custom Client"
    },
    "requestId": "id",
    "routeKey": "$default",
    "stage": "$default",
    "time": "12/Mar/2026:19:03:58 +0000",
    "timeEpoch": 1741806238000
  },
  "body": $body,
  "isBase64Encoded": false
}')

echo "   Invoking with URL: $TEST_AUDIO_URL"
echo "--------------------------------------------------------"

cargo lambda invoke --data-ascii "$WRAPPED_PAYLOAD"

echo ""
echo "--------------------------------------------------------"
echo "✅ Test finished."
