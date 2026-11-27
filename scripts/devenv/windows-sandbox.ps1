<#
.SYNOPSIS
    Start this checkout's Windows build in Windows Sandbox: a clean, throwaway
    Windows that is wiped when you close it.

.DESCRIPTION
    Every launch is a fresh install with nothing on it (no Visual C++ runtime,
    no Rust, no drivers beyond Windows' own), which is the machine a user
    downloading the editor has. dist/windows-x64 is mapped into the sandbox
    rather than copied, so a rebuild on the host is what the next sandbox runs.

    The sandbox shares this PC's GPU (vGPU), so the editor renders for real.

    Windows Sandbox is a Windows Pro feature; host-setup.ps1 turns it on.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\devenv\windows-sandbox.ps1
    # Start the game runtime instead of the editor:
    powershell -ExecutionPolicy Bypass -File scripts\devenv\windows-sandbox.ps1 -Arguments '--no-editor'
#>
param(
    [string]$Build = (Join-Path (Resolve-Path "$PSScriptRoot\..\..").Path 'dist\windows-x64'),
    # Passed to renzora.exe inside the sandbox.
    [string]$Arguments = '',
    [int]$MemoryMB = 8192
)
$ErrorActionPreference = 'Stop'

if (-not (Test-Path (Join-Path $Build 'renzora.exe'))) {
    throw "No Windows build at $Build. Build one first (cargo renzora dist, or renzora build windows)."
}
if (-not (Get-Command WindowsSandbox.exe -ErrorAction SilentlyContinue)) {
    throw "Windows Sandbox is not turned on. Run scripts\devenv\host-setup.ps1 as Administrator, then reboot."
}

# Inside the sandbox the build is at C:\renzora. Read-write, because the editor
# writes beside itself (logs, a first launch's unpacked SDK); a sandbox cannot
# change anything else on this PC.
$exe = 'C:\renzora\renzora.exe'
$command = if ($Arguments) { "$exe $Arguments" } else { $exe }
$config = @"
<Configuration>
  <vGPU>Enable</vGPU>
  <Networking>Enable</Networking>
  <MemoryInMB>$MemoryMB</MemoryInMB>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$Build</HostFolder>
      <SandboxFolder>C:\renzora</SandboxFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>$command</Command>
  </LogonCommand>
</Configuration>
"@
$wsb = Join-Path $env:TEMP 'renzora-sandbox.wsb'
Set-Content -Path $wsb -Value $config -Encoding UTF8
Write-Host "Starting Windows Sandbox with $Build mapped to C:\renzora"
Start-Process WindowsSandbox.exe -ArgumentList "`"$wsb`""
