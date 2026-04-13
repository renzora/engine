# Build the marketplace preview WASM module.
#
# Usage:
#   .\build-preview.ps1                    # Build to .\dist\
#   .\build-preview.ps1 C:\path\to\output  # Build to custom output dir
#
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli

param(
    [string]$OutputDir = "$PSScriptRoot\dist"
)

$ErrorActionPreference = "Stop"

Write-Host "[preview] Building renzora_preview for wasm32-unknown-unknown..."
Push-Location "$PSScriptRoot\renzora_preview"

try {
    cargo build --release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed" }

    Write-Host "[preview] Running wasm-bindgen..."
    New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

    wasm-bindgen `
        --out-dir $OutputDir `
        --target web `
        --no-typescript `
        "$PSScriptRoot\renzora_preview\target\wasm32-unknown-unknown\release\renzora_preview.wasm"
    if ($LASTEXITCODE -ne 0) { throw "wasm-bindgen failed" }

    # Optimize with wasm-opt if available
    $wasmFile = Join-Path $OutputDir "renzora_preview_bg.wasm"
    $sizeBefore = (Get-Item $wasmFile).Length / 1MB

    if (Get-Command wasm-opt -ErrorAction SilentlyContinue) {
        Write-Host "[preview] Optimizing with wasm-opt..."
        wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int -o $wasmFile $wasmFile
        $sizeAfter = (Get-Item $wasmFile).Length / 1MB
        Write-Host ("[preview] wasm-opt: {0:N1}MB -> {1:N1}MB" -f $sizeBefore, $sizeAfter)
    } else {
        Write-Host "[preview] wasm-opt not found, skipping optimization"
    }

    Write-Host "[preview] Done! Output in $OutputDir"
    Get-ChildItem $OutputDir | Format-Table Name, @{N='Size';E={"{0:N1} MB" -f ($_.Length / 1MB)}}
}
finally {
    Pop-Location
}
