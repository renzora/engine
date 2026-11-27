#Requires -RunAsAdministrator
<#
.SYNOPSIS
    One-time setup on the Windows host so Hyper-V VMs can run the builds in this
    checkout's dist/ folder.

.DESCRIPTION
    Every OS builds into dist/<platform>/ of this one checkout, and each test VM
    opens that folder over the network, so a rebuild on the host is what every VM
    runs next. This script does the parts that need Administrator, once:

      1. A local account, `engine`, that the VMs sign in to the share with. Not
         your own account: on a Microsoft account that would mean typing your
         Microsoft password (not your PIN) into every VM. It is hidden from the
         sign-in screen and can only reach this checkout.
      2. An SMB share of this checkout, named `engine`, for that account and you.
      3. A firewall rule for file sharing on the Hyper-V Default Switch only.
         That network is internal to this PC; nothing on your LAN can reach it.
      4. Windows Sandbox, the throwaway clean Windows used to test Windows builds
         (see windows-sandbox.wsb). Needs a reboot the first time.

    Safe to run again: each step checks what is already there.

.EXAMPLE
    # In PowerShell opened with "Run as administrator", from the checkout:
    powershell -ExecutionPolicy Bypass -File scripts\devenv\host-setup.ps1
#>
param(
    # The checkout to share. Defaults to the one this script lives in.
    [string]$Repo = (Resolve-Path "$PSScriptRoot\..\..").Path,
    [string]$ShareName = 'engine',
    [string]$Account = 'engine',
    # Skip turning on Windows Sandbox.
    [switch]$NoSandbox
)
$ErrorActionPreference = 'Stop'

Write-Host "Sharing $Repo as \\$env:COMPUTERNAME.mshome.net\$ShareName" -ForegroundColor Cyan

# ── 1. The share account ────────────────────────────────────────────────────
if (Get-LocalUser -Name $Account -ErrorAction SilentlyContinue) {
    Write-Host "[1/4] account '$Account' already exists (run with a new password? use: net user $Account *)"
} else {
    $password = Read-Host "[1/4] Choose a password for the '$Account' share account" -AsSecureString
    New-LocalUser -Name $Account -Password $password -PasswordNeverExpires `
        -UserMayNotChangePassword -AccountNeverExpires `
        -Description 'Signs Hyper-V VMs in to the engine share' | Out-Null
    # Off the sign-in screen: it exists for the share, not for logging in.
    $hide = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList'
    New-Item -Path $hide -Force | Out-Null
    New-ItemProperty -Path $hide -Name $Account -Value 0 -PropertyType DWord -Force | Out-Null
    Write-Host "[1/4] created account '$Account'"
}

# Read and write on the checkout (the VMs run builds from it, and an editor
# writes logs and caches beside itself). Nothing outside it. Inherited, so it
# covers files a later build creates; applying it walks every existing file,
# target/ included, which is minutes on a large checkout.
Write-Host "      granting '$Account' access to the checkout (can take several minutes)..."
icacls $Repo /grant "${Account}:(OI)(CI)M" /T /C /Q | Out-Null

# ── 2. The share ────────────────────────────────────────────────────────────
$existing = Get-SmbShare -Name $ShareName -ErrorAction SilentlyContinue
if ($existing -and $existing.Path -ne $Repo) {
    throw "A share named '$ShareName' already points at $($existing.Path). Remove it or pass -ShareName."
}
if (-not $existing) {
    New-SmbShare -Name $ShareName -Path $Repo -FullAccess $Account, $env:USERNAME `
        -Description 'Renzora engine checkout for test VMs' | Out-Null
    Write-Host "[2/4] shared $Repo as '$ShareName'"
} else {
    Write-Host "[2/4] share '$ShareName' already exists"
}

# ── 3. Firewall, Hyper-V switch only ────────────────────────────────────────
$rule = 'Renzora engine share (Hyper-V Default Switch)'
if (-not (Get-NetFirewallRule -DisplayName $rule -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -DisplayName $rule -Direction Inbound -Protocol TCP -LocalPort 445 `
        -InterfaceAlias 'vEthernet (Default Switch)' -Action Allow -Profile Any | Out-Null
    Write-Host "[3/4] allowed file sharing on the Hyper-V Default Switch"
} else {
    Write-Host "[3/4] firewall rule already present"
}

# ── 4. Windows Sandbox ──────────────────────────────────────────────────────
if ($NoSandbox) {
    Write-Host "[4/4] skipped Windows Sandbox"
} else {
    $feature = Get-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM
    if ($feature.State -eq 'Enabled') {
        Write-Host "[4/4] Windows Sandbox already on"
    } else {
        Enable-WindowsOptionalFeature -Online -FeatureName Containers-DisposableClientVM -All -NoRestart | Out-Null
        Write-Host "[4/4] turned on Windows Sandbox: REBOOT before using it" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "Done. In a VM, connect to:  \\$env:COMPUTERNAME.mshome.net\$ShareName" -ForegroundColor Green
Write-Host "  user: $env:COMPUTERNAME\$Account   (or .\$Account)   password: the one you chose"
