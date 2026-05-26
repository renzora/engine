#!/usr/bin/env pwsh
# Windows implementation of the `dev` launcher (invoked by the root `dev.cmd`).
# Everything runs inside the `renzora/engine` container, so the host needs only
# Docker — one pinned toolchain/environment (the ABI contract the dlopen plugin
# system depends on), no native Rust.

$ErrorActionPreference = "Stop"
$Image = "renzora/engine"

# This script lives in docker/, so the repo root is its parent. Anchor there so
# `dev` always targets the repo regardless of the caller's cwd.
$Root = if ($PSScriptRoot) { Split-Path $PSScriptRoot -Parent } else { (Get-Location).Path }
Set-Location $Root
$Dockerfile = "docker/engine-builder/Dockerfile"

$md5 = [System.Security.Cryptography.MD5]::Create()
$bytes = [System.Text.Encoding]::UTF8.GetBytes($Root)
$hash = [System.BitConverter]::ToString($md5.ComputeHash($bytes)).Replace("-", "").Substring(0, 8).ToLower()
$Name = "renzora-$hash"

# Vendored-crate excludes — mirror .github/workflows/test.yml so local and CI agree.
$Excludes = "--exclude renzora_shader --exclude bevy_gauge --exclude bevy_hanabi --exclude bevy_mod_outline --exclude bevy_silk --exclude vleue_navigator --exclude bevy_mod_openxr --exclude bevy_mod_xr --exclude bevy_xr_utils"

function Ensure-Up {
    if (-not (docker images -q $Image)) {
        Write-Host "Building image $Image (first time, this takes a while)..."
        docker build -f $Dockerfile -t $Image .
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    $imageId = docker image inspect $Image --format "{{.Id}}"
    if ((docker ps -aq -f "name=^$Name$") -and -not (docker ps -aq -f "name=^$Name$" -f "label=renzora.image=$imageId")) {
        Write-Host "Container $Name is from an outdated image - recreating..."
        docker rm -f $Name | Out-Null
    }
    if (-not (docker ps -aq -f "name=^$Name$")) {
        Write-Host "Creating container $Name..."
        docker create --name $Name --label "renzora.image=$imageId" -v "${Root}:/app/src" -w /app/src $Image sleep infinity | Out-Null
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    docker start $Name | Out-Null
}

function Dexec($cmd) {
    docker exec $Name bash -c $cmd
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$verb = if ($args.Count -ge 1) { $args[0] } else { "help" }
$rest = if ($args.Count -ge 2) { ($args[1..($args.Count - 1)]) -join " " } else { "" }

switch ($verb) {
    "init" { Ensure-Up; Write-Host "Container $Name is running." }
    "build" { Ensure-Up; Dexec "/app/src/docker/scripts/build-all.sh dist $rest" }
    "test" {
        Ensure-Up
        if ($rest) { Dexec "cargo test $rest" } else { Dexec "cargo test --workspace $Excludes" }
    }
    "check" {
        Ensure-Up
        if ($rest) { Dexec "cargo check $rest" } else { Dexec "cargo check --workspace $Excludes" }
    }
    "run" {
        Ensure-Up
        $feature = if ($rest) { $rest.Trim() } else { "editor" }
        if ($feature -ne "editor" -and $feature -ne "runtime") {
            Write-Host "Usage: dev run [editor|runtime]"; exit 1
        }
        # Cross-build the Windows binary in the container; output lands in the
        # bind-mounted dist/ (host-visible), then we run it natively on the GPU.
        Dexec "/app/src/docker/scripts/build-all.sh dist windows"
        $exe = if ($feature -eq "editor") { "renzora.exe" } else { "renzora-runtime.exe" }
        $dir = Join-Path $Root "dist\windows-x64\$feature"
        $path = Join-Path $dir $exe
        if (-not (Test-Path $path)) { Write-Host "Built binary not found: $path"; exit 1 }
        Write-Host "Running $path ..."
        # Run from the binary's own dir so it finds its assets/plugins.
        Push-Location $dir
        try { & ".\$exe" } finally { Pop-Location }
    }
    "add" { Ensure-Up; Dexec "bash docker/scripts/add-plugin.sh $rest" }
    "remove" { Ensure-Up; Dexec "bash docker/scripts/remove-plugin.sh $rest" }
    "upx" { Ensure-Up; Dexec "bash docker/scripts/upx-compress.sh $rest" }
    "shell" { Ensure-Up; docker exec -it $Name bash }
    "clean" { Ensure-Up; Dexec "rm -rf target && echo 'target/ cleaned'" }
    "destroy" { docker rm -f $Name 2>$null | Out-Null; Write-Host "Removed container $Name." }
    default {
        Write-Host @"
Renzora dev launcher - everything runs in the renzora/engine container.
Usage: dev <command> [args]
  init                  build image + create/start container (idempotent)
  build [platforms]     cross-build via docker/scripts/build-all.sh (no args = all)
  test  [args]          cargo test in the container (no args = workspace suite)
  check [args]          cargo check in the container
  run   [editor|runtime]  build for this host + run it (editor default)
  add   <name> [--editor|--dylib]   scaffold a new plugin crate
  remove <name>         delete a plugin crate
  upx   [platforms]     UPX-compress built binaries under dist/
  shell                 interactive bash in the container
  clean                 remove target/ in the container
  destroy               remove the container
"@
    }
}
