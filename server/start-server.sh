#!/bin/bash
echo "🚀 Starting Renzora WebSocket Server..."
echo

# Check if we're in the server directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Cargo.toml not found. Please run this from the server directory."
    echo "   Expected: server/start-server.sh"
    exit 1
fi

# Set environment variables if needed
# export RENZORA_BASE_PATH="/path/to/your/engine"
# export RENZORA_PROJECTS_PATH="/path/to/your/projects"
# export RENZORA_PORT="3002"
# export RUST_LOG="info"

echo "📋 Configuration:"
echo "   Base Path: ${RENZORA_BASE_PATH:-auto-detect}"
echo "   Projects:  ${RENZORA_PROJECTS_PATH:-auto-detect}"
echo "   Port:      ${RENZORA_PORT:-3002}"
echo "   Log Level: ${RUST_LOG:-info}"
echo

echo "🔨 Building and starting server..."
cargo run