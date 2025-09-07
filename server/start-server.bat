@echo off
echo 🚀 Starting Renzora WebSocket Server...
echo.

REM Check if we're in the server directory
if not exist "Cargo.toml" (
    echo ❌ Error: Cargo.toml not found. Please run this from the server directory.
    echo    Expected: server/start-server.bat
    pause
    exit /b 1
)

REM Set environment variables if needed
REM set RENZORA_BASE_PATH=C:\path\to\your\engine
REM set RENZORA_PROJECTS_PATH=C:\path\to\your\projects
REM set RENZORA_PORT=3002
REM set RUST_LOG=info

echo 📋 Configuration:
echo    Base Path: %RENZORA_BASE_PATH%
echo    Projects:  %RENZORA_PROJECTS_PATH%  
echo    Port:      %RENZORA_PORT%
echo    Log Level: %RUST_LOG%
echo.

echo 🔨 Building and starting server...
cargo run

pause