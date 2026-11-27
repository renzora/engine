<#
.SYNOPSIS
    Download and prepare everything the macOS Hyper-V VM boots from. No
    Administrator needed; create-vm.ps1 then builds the VM from this folder.

.DESCRIPTION
    Into -Dir:
      efi\                              OSX-Hyper-V's prebuilt OpenCore EFI (pinned
                                        release), with the AMD patches applied when
                                        this PC has an AMD CPU
      recovery\com.apple.recovery.boot  the macOS recovery image, from Apple

    Every download is pinned: the OSX-Hyper-V release by version and the AMD
    patch set by commit. The release's own scripts download and run an unpinned
    bootstrap from GitHub as Administrator; this does the same work without it.

    Needs Python 3 on PATH (apply-amd.py and Apple's macrecovery.py use only its
    standard library).

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\devenv\macos\prepare.ps1 -Dir V:\VMs\osx-hyper-v
#>
param(
    [Parameter(Mandatory)][string]$Dir,
    # Must match create-vm.ps1 -Cores: the AMD patches bake the core count in.
    [int]$Cores = 8,
    # macOS 15 Sequoia. OSX-Hyper-V 0.4.0 does not support 26 yet.
    [ValidateSet('13', '14', '15')][string]$Version = '15'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$release = '0.4.0'
$efiZip = "https://github.com/Qonfused/OSX-Hyper-V/releases/download/$release/EFI-$release-64-bit-RELEASE.zip"
$amdCommit = 'eaf52ef292abf4ebec899df6d48626569ba50cc6'
$amdPatches = "https://raw.githubusercontent.com/AMD-OSX/AMD_Vanilla/$amdCommit/patches.plist"
# Board ids from the release's create-macos-recovery.ps1.
$board = @{ '13' = 'Mac-B4831CEBD52A0C4C'; '14' = 'Mac-827FAC58A8FDFA22'; '15' = 'Mac-7BA5B2D9E42DDD94' }[$Version]

if (-not (Get-Command python -ErrorAction SilentlyContinue)) { throw 'Python 3 is needed on PATH.' }
New-Item -ItemType Directory -Force $Dir | Out-Null
$Dir = (Resolve-Path $Dir).Path

# ── 1. The OpenCore EFI ─────────────────────────────────────────────────────
if (Test-Path "$Dir\efi\EFI\OC\config.plist") {
    Write-Host "[1/3] EFI already in $Dir\efi"
} else {
    Write-Host "[1/3] downloading OSX-Hyper-V $release"
    Invoke-WebRequest $efiZip -OutFile "$Dir\efi.zip" -UseBasicParsing
    Expand-Archive "$Dir\efi.zip" -DestinationPath "$Dir\efi" -Force
    Remove-Item "$Dir\efi.zip"
}

# ── 2. AMD patches ──────────────────────────────────────────────────────────
$cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Manufacturer
if ($cpu -eq 'AuthenticAMD') {
    Write-Host "[2/3] AMD CPU: applying the AMD patches for $Cores cores"
    New-Item -ItemType Directory -Force "$Dir\amd" | Out-Null
    Invoke-WebRequest $amdPatches -OutFile "$Dir\amd\patches.plist" -UseBasicParsing
    # Always patches the untouched config.plist.orig, so re-running with a
    # different -Cores replaces the patches instead of stacking them.
    & python -I "$PSScriptRoot\apply-amd.py" "$Dir\efi\EFI\OC\config.plist" "$Dir\amd\patches.plist" $Cores
    if ($LASTEXITCODE) { throw 'apply-amd.py failed' }
} else {
    Write-Host "[2/3] $cpu CPU: no AMD patches needed"
}

# ── 3. The recovery image ───────────────────────────────────────────────────
$recovery = "$Dir\recovery\com.apple.recovery.boot"
if (Test-Path "$recovery\BaseSystem.dmg") {
    Write-Host "[3/3] recovery image already in $recovery"
} else {
    Write-Host "[3/3] downloading the macOS $Version recovery image from Apple (about 850 MB)"
    New-Item -ItemType Directory -Force $recovery | Out-Null
    # In a console window of its own: macrecovery.py sizes its progress bar from
    # the terminal and crashes ("The handle is invalid") without one.
    $p = Start-Process python -Wait -PassThru -WindowStyle Minimized -WorkingDirectory $env:TEMP -ArgumentList @(
        '-I', "$Dir\efi\Utilities\macrecovery\macrecovery.py",
        '-b', $board, '-m', '00000000000000000', '-o', $recovery, 'download')
    if ($p.ExitCode -or -not (Test-Path "$recovery\BaseSystem.dmg")) { throw "macrecovery.py failed (exit $($p.ExitCode))" }
}

Write-Host ""
Write-Host "Ready. Next, as Administrator:" -ForegroundColor Green
Write-Host "  powershell -ExecutionPolicy Bypass -File scripts\devenv\macos\create-vm.ps1 -Source `"$Dir`" -Cores $Cores"
