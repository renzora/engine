@echo off
echo 🚀 Building Renzora Server for Release...
echo.

REM Check if we're in the server directory
if not exist "Cargo.toml" (
    echo ❌ Error: Cargo.toml not found. Please run this from the server directory.
    pause
    exit /b 1
)

echo 🔨 Building optimized release binary...
cargo build --release

if errorlevel 1 (
    echo ❌ Build failed!
    pause
    exit /b 1
)

echo.
echo ✅ Build successful! 
echo 📁 Binary location: target\release\renzora-server.exe
echo.
echo 🚀 To run the server:
echo    1. Copy renzora-server.exe to any directory
echo    2. Create renzora.toml config file (optional)
echo    3. Run: renzora-server.exe
echo.
echo 📋 The server will auto-detect the engine directory by looking for:
echo    - package.json
echo    - src/ directory
echo    - bridge/ directory
echo.

pause