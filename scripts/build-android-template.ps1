# Build Android runtime template APKs (one per architecture).
#
# Prerequisites (install once):
#   1. Android Studio (includes SDK, NDK, Java)
#   2. cargo-ndk:  cargo install cargo-ndk
#   3. Rust Android targets:
#        rustup target add aarch64-linux-android --toolchain nightly
#        rustup target add x86_64-linux-android --toolchain nightly
#        rustup target add armv7-linux-androideabi --toolchain nightly
#
# Usage:
#   .\scripts\build-android-template.ps1                        # Android ARM64 (Vulkan)
#   .\scripts\build-android-template.ps1 --x86_64               # Android x86_64 (Vulkan)
#   .\scripts\build-android-template.ps1 --firetv-arm           # Fire TV ARM 32-bit (GLES)
#   .\scripts\build-android-template.ps1 --firetv-arm64         # Fire TV ARM64 (Vulkan)
#   .\scripts\build-android-template.ps1 --firetv               # Both Fire TV templates
#   .\scripts\build-android-template.ps1 --all                  # Build all templates
#   .\scripts\build-android-template.ps1 --firetv-arm --x86_64  # Multiple targets

param(
    [switch]$arm64,
    [Alias("x86_64")][switch]$x86,
    [Alias("firetv-arm")][switch]$firetvArm,
    [Alias("firetv-arm64")][switch]$firetvArm64,
    [switch]$firetv,
    [switch]$all
)

$ErrorActionPreference = "Stop"

$ProjectRoot = (Resolve-Path "$PSScriptRoot\..").Path
$AndroidDir = "$ProjectRoot\android"
$AndroidCrate = "$ProjectRoot\crates\platform\renzora_android"
$JniLibsDir = "$AndroidDir\app\src\main\jniLibs"

# --- Expand shortcut flags ---
if ($firetv) { $firetvArm = $true; $firetvArm64 = $true }
if ($all) { $arm64 = $true; $x86 = $true; $firetvArm = $true; $firetvArm64 = $true }

# Default: Android ARM64 if nothing specified
if (-not $arm64 -and -not $x86 -and -not $firetvArm -and -not $firetvArm64) {
    $arm64 = $true
}

# --- Auto-detect environment ---

# Java
if (-not $env:JAVA_HOME) {
    $jbrPath = "$env:ProgramFiles\Android\Android Studio\jbr"
    if (Test-Path $jbrPath) {
        $env:JAVA_HOME = $jbrPath
    } else {
        Write-Error "JAVA_HOME not set and Android Studio JBR not found. Set JAVA_HOME or install Android Studio."
    }
}
Write-Host "JAVA_HOME: $env:JAVA_HOME"

# Android SDK
if (-not $env:ANDROID_HOME) {
    $sdkPath = "$env:LOCALAPPDATA\Android\Sdk"
    if (Test-Path $sdkPath) {
        $env:ANDROID_HOME = $sdkPath
    } else {
        Write-Error "ANDROID_HOME not set and Android SDK not found."
    }
}
Write-Host "ANDROID_HOME: $env:ANDROID_HOME"

# Android NDK
if (-not $env:ANDROID_NDK_HOME) {
    $ndkDir = "$env:ANDROID_HOME\ndk"
    if (Test-Path $ndkDir) {
        $latest = Get-ChildItem $ndkDir -Directory | Sort-Object Name | Select-Object -Last 1
        $env:ANDROID_NDK_HOME = $latest.FullName
    } else {
        Write-Error "No NDK found in $ndkDir. Install via Android Studio SDK Manager."
    }
}
Write-Host "ANDROID_NDK_HOME: $env:ANDROID_NDK_HOME"

# Ensure local.properties exists for Gradle
$localProps = "$AndroidDir\local.properties"
if (-not (Test-Path $localProps)) {
    $sdkEscaped = $env:ANDROID_HOME -replace '\\', '\\\\'
    Set-Content -Path $localProps -Value "sdk.dir=$sdkEscaped"
}

# Check cargo-ndk
if (-not (Get-Command cargo-ndk -ErrorAction SilentlyContinue)) {
    Write-Error "cargo-ndk not found. Install with: cargo install cargo-ndk"
}

# NDK sysroot for libc++_shared.so
$ndkPrebuilt = "$env:ANDROID_NDK_HOME\toolchains\llvm\prebuilt"
$hostDir = (Get-ChildItem $ndkPrebuilt -Directory | Select-Object -First 1).Name
$ndkLibs = "$ndkPrebuilt\$hostDir\sysroot\usr\lib"

# Gradle command
$gradleCmd = if (Test-Path "$AndroidDir\gradlew.bat") { "$AndroidDir\gradlew.bat" } else { "$AndroidDir\gradlew" }

# Templates directory
$templatesDir = "$env:APPDATA\renzora\templates"
New-Item -ItemType Directory -Force -Path $templatesDir | Out-Null

$outputDir = "$ProjectRoot\build\templates"
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

# --- Helper: build one architecture ---

function Build-Arch {
    param(
        [string]$RustTarget,
        [string]$Abi,
        [string]$TemplateName,
        [string]$Flavor,
        [bool]$UseGles = $false,
        [int]$MinPlatform = 30
    )

    $flavorCap = $Flavor.Substring(0,1).ToUpper() + $Flavor.Substring(1)
    $task = "assemble${flavorCap}Release"
    $apkPath = "$AndroidDir\app\build\outputs\apk\$Flavor\release\app-${Flavor}-release-unsigned.apk"

    Write-Host ""
    Write-Host "=== Building $TemplateName ===" -ForegroundColor Cyan
    Write-Host "    Arch: $Abi | Flavor: $Flavor | API: $MinPlatform"
    Write-Host ""

    # Build native library
    Write-Host "--- Building native library: $Abi ---"
    Push-Location $AndroidCrate

    $extraArgs = @()
    if ($UseGles) {
        $extraArgs = @("--features", "gles")
        Write-Host "  -> Using OpenGL ES backend" -ForegroundColor Yellow
    } else {
        Write-Host "  -> Using Vulkan backend" -ForegroundColor Green
    }

    & cargo ndk --target $RustTarget --platform $MinPlatform build --release @extraArgs
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error "cargo ndk build failed" }

    Pop-Location

    # Clean jniLibs and copy only this arch
    if (Test-Path $JniLibsDir) { Remove-Item -Recurse -Force $JniLibsDir }
    New-Item -ItemType Directory -Force -Path "$JniLibsDir\$Abi" | Out-Null

    Copy-Item "$AndroidCrate\target\$RustTarget\release\libmain.so" "$JniLibsDir\$Abi\libmain.so"

    # NDK uses different directory names than Rust targets for some archs
    $ndkTarget = $RustTarget
    if ($RustTarget -eq "armv7-linux-androideabi") { $ndkTarget = "arm-linux-androideabi" }
    Copy-Item "$ndkLibs\$ndkTarget\libc++_shared.so" "$JniLibsDir\$Abi\"

    Write-Host "  -> $Abi`: libmain.so + libc++_shared.so"

    # Build APK
    Write-Host ""
    Write-Host "--- Building APK: $task ---"
    Push-Location $AndroidDir

    & $gradleCmd ":app:$task"
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Error "Gradle build failed" }

    Pop-Location

    if (-not (Test-Path $apkPath)) {
        Write-Error "APK not found at $apkPath"
    }

    Copy-Item $apkPath "$templatesDir\$TemplateName"
    Copy-Item $apkPath "$outputDir\$TemplateName"
    Write-Host ""
    Write-Host "  Template: $templatesDir\$TemplateName" -ForegroundColor Green
}

# --- Build selected targets ---

if ($arm64) {
    Build-Arch -RustTarget "aarch64-linux-android" -Abi "arm64-v8a" -TemplateName "renzora-runtime-android-arm64.apk" -Flavor "standard" -UseGles $false -MinPlatform 30
}

if ($x86) {
    Build-Arch -RustTarget "x86_64-linux-android" -Abi "x86_64" -TemplateName "renzora-runtime-android-x86_64.apk" -Flavor "standard" -UseGles $false -MinPlatform 30
}

if ($firetvArm) {
    Build-Arch -RustTarget "armv7-linux-androideabi" -Abi "armeabi-v7a" -TemplateName "renzora-runtime-firetv-arm.apk" -Flavor "firetv" -UseGles $true -MinPlatform 25
}

if ($firetvArm64) {
    Build-Arch -RustTarget "aarch64-linux-android" -Abi "arm64-v8a" -TemplateName "renzora-runtime-firetv-arm64.apk" -Flavor "firetv" -UseGles $false -MinPlatform 25
}

# --- Clean up ---

if (Test-Path $JniLibsDir) { Remove-Item -Recurse -Force $JniLibsDir }

Write-Host ""
Write-Host "=== Done! ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Export from the editor to build a signed APK ready to install."
Write-Host ""
