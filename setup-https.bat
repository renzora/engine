@echo off
echo Setting up HTTPS for localhost development...

REM Check if mkcert exists
if not exist "mkcert.exe" (
    echo Downloading mkcert...
    powershell -Command "Invoke-WebRequest -Uri 'https://github.com/FiloSottile/mkcert/releases/download/v1.4.4/mkcert-v1.4.4-windows-amd64.exe' -OutFile 'mkcert.exe'"
    if errorlevel 1 (
        echo Failed to download mkcert
        echo Please download manually from: https://github.com/FiloSottile/mkcert/releases
        pause
        exit /b 1
    )
    echo mkcert downloaded successfully
)

echo Installing local Certificate Authority...
echo NOTE: You may see a security prompt - click YES to install the local CA
mkcert.exe -install
if errorlevel 1 (
    echo Failed to install local CA
    echo Make sure to run this script as Administrator
    pause
    exit /b 1
)

echo Generating certificates for localhost...
mkcert.exe localhost 127.0.0.1 ::1
if errorlevel 1 (
    echo Failed to generate certificates
    pause
    exit /b 1
)

echo.
echo Setup complete!
echo Your certificates are ready:
echo   - localhost+2.pem (certificate)
echo   - localhost+2-key.pem (private key)
echo.
echo IMPORTANT: Restart your server to enable HTTPS:
echo   1. Stop your current server (Ctrl+C)
echo   2. Run: npm run dev
echo   3. Your app will be at: https://localhost:3000
echo.
echo WebGPU should now work in development!
echo.
pause