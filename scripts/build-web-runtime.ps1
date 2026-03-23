$ErrorActionPreference = "Stop"

# Build WASM runtime
cargo dist-web-runtime
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Generate JS bindings
wasm-bindgen --out-dir target/dist/web --out-name renzora-runtime --target web target/wasm32-unknown-unknown/dist/renzora-runtime.wasm
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Optimize WASM
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals -o target/dist/web/renzora-runtime_bg.wasm target/dist/web/renzora-runtime_bg.wasm
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Brotli compress
$brotli = "C:\Program Files\Git\mingw64\bin\brotli.exe"
& $brotli -k -f -Z target/dist/web/renzora-runtime_bg.wasm
if ($LASTEXITCODE -ne 0) { Write-Warning "Brotli compression failed, skipping .br" }

# Package zip
$zipPath = "target\dist\renzora-runtime-web-wasm32.zip"
if (Test-Path $zipPath) { Remove-Item $zipPath }

$files = @(
    "target\dist\web\renzora-runtime.js",
    "target\dist\web\renzora-runtime_bg.wasm"
)
if (Test-Path "target\dist\web\renzora-runtime_bg.wasm.br") {
    $files += "target\dist\web\renzora-runtime_bg.wasm.br"
}

# Use .NET directly since Compress-Archive may not be available
Add-Type -Assembly System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::Open((Resolve-Path -Path "." | Join-Path -ChildPath $zipPath), 'Create')
foreach ($f in $files) {
    $name = Split-Path $f -Leaf
    [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $f, $name) | Out-Null
}
$zip.Dispose()

# Copy to templates
$templatesDir = "$env:APPDATA\renzora\templates"
if (-not (Test-Path $templatesDir)) { New-Item -ItemType Directory -Path $templatesDir | Out-Null }
Copy-Item $zipPath "$templatesDir\renzora-runtime-web-wasm32.zip"

Write-Output "Template: $zipPath"
