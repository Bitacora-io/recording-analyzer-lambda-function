#!/bin/bash

# Exit immediately if a command fails
set -e

echo "🔍 Preparing local test environment..."

# 1. Load API Key from .env file if it exists
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
    echo "✅ Environment variables loaded from .env"
else
    echo "⚠️ .env file not found. Using system environment variables."
fi

# Validate that the API key is configured
if [ -z "$GEMINI_API_KEY" ] || [ "$GEMINI_API_KEY" == "" ]; then
    echo "❌ Error: GEMINI_API_KEY variable is invalid or not configured."
    echo "Please create a .env file by copying .env.example and place your real API Key:"
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
TEST_AUDIO_URL="https://firebasestorage.googleapis.com/v0/b/bitacora-657e2.appspot.com/o/vertex_temp%2FGMT20260212-190237_Recording.m4a?alt=media&token=4de20a19-2ecc-4815-9895-c4fe4935eeae"

echo "   Invoking with URL: $TEST_AUDIO_URL"
echo "--------------------------------------------------------"

cargo lambda invoke --data-ascii "{ \"audio_url\": \"$TEST_AUDIO_URL\" }"

echo ""
echo "--------------------------------------------------------"
echo "✅ Test finished."
