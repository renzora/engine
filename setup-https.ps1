# PowerShell script to setup HTTPS for localhost development
# Run this script as Administrator

Write-Host "Setting up HTTPS for localhost development..." -ForegroundColor Green

# Check if mkcert exists
if (!(Test-Path ".\mkcert.exe")) {
    Write-Host "Downloading mkcert..." -ForegroundColor Yellow
    
    # Download mkcert
    $url = "https://github.com/FiloSottile/mkcert/releases/download/v1.4.4/mkcert-v1.4.4-windows-amd64.exe"
    try {
        Invoke-WebRequest -Uri $url -OutFile "mkcert.exe"
        Write-Host "✓ mkcert downloaded successfully" -ForegroundColor Green
    } catch {
        Write-Host "✗ Failed to download mkcert. Please download manually from:" -ForegroundColor Red
        Write-Host "  https://github.com/FiloSottile/mkcert/releases" -ForegroundColor Yellow
        exit 1
    }
}

# Install the local CA (requires Administrator)
Write-Host "Installing local Certificate Authority..." -ForegroundColor Yellow
try {
    & .\mkcert.exe -install
    Write-Host "✓ Local CA installed successfully" -ForegroundColor Green
} catch {
    Write-Host "✗ Failed to install local CA. Make sure you're running as Administrator" -ForegroundColor Red
    Write-Host "  Try running: Right-click PowerShell -> Run as Administrator" -ForegroundColor Yellow
}

# Generate certificates for localhost
Write-Host "Generating certificates for localhost..." -ForegroundColor Yellow
try {
    & .\mkcert.exe localhost 127.0.0.1 ::1
    Write-Host "✓ Certificates generated successfully" -ForegroundColor Green
    Write-Host "  - localhost+2.pem (certificate)" -ForegroundColor Cyan
    Write-Host "  - localhost+2-key.pem (private key)" -ForegroundColor Cyan
} catch {
    Write-Host "✗ Failed to generate certificates" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "🎉 Setup complete!" -ForegroundColor Green
Write-Host "Now you can run your dev server with HTTPS:" -ForegroundColor White
Write-Host "  npm run dev" -ForegroundColor Cyan
Write-Host ""
Write-Host "Your app will be available at:" -ForegroundColor White
Write-Host "  https://localhost:3000" -ForegroundColor Cyan
Write-Host ""
Write-Host "WebGPU should now work in development! 🚀" -ForegroundColor Green