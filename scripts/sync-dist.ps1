$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Force -Path dist | Out-Null

# --- Clean stale build outputs ---
Get-ChildItem dist/*.exe -ErrorAction SilentlyContinue | Remove-Item -Force
Get-ChildItem dist/*.dll -ErrorAction SilentlyContinue | Remove-Item -Force
if (Test-Path dist/plugins) { Remove-Item dist/plugins -Recurse -Force }

# --- Copy new build outputs ---

# Binaries
Copy-Item target/dist/*.exe dist/ -Force -ErrorAction SilentlyContinue

# bevy_dylib with hash from deps
Copy-Item target/dist/deps/bevy_dylib-*.dll dist/ -Force -ErrorAction SilentlyContinue

# Rust std from toolchain
$sysroot = (rustc --print sysroot)
Copy-Item (Join-Path $sysroot "bin/std-*.dll") dist/ -Force -ErrorAction SilentlyContinue

# SDK DLLs (needed for community plugins to link against)
foreach ($dll in @("renzora.dll", "renzora_runtime.dll")) {
    if (Test-Path "target/dist/$dll") {
        Copy-Item "target/dist/$dll" "dist/$dll" -Force
    }
}

# --- Summary ---
$count = (Get-ChildItem dist/*.exe, dist/*.dll -ErrorAction SilentlyContinue).Count
Write-Output "Synced to dist/ - $count files"
