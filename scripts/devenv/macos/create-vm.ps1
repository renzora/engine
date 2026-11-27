#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Create the macOS Hyper-V VM from a folder prepare.ps1 filled.

.DESCRIPTION
    Same disks and settings as OSX-Hyper-V's create-virtual-machine.ps1 and
    convert-efi-disk.ps1: a Generation 2 VM with Secure Boot off, booting a FAT32
    EFI disk that carries OpenCore and the recovery image, plus an install disk
    the macOS installer erases to APFS. Administrator is needed to format the
    virtual disks.

    Memory is fixed, not dynamic: macOS has no Hyper-V dynamic memory driver.
    The installer needs 6 to 8 GB free when the VM starts.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\devenv\macos\create-vm.ps1 -Source V:\VMs\osx-hyper-v
#>
param(
    [Parameter(Mandatory)][string]$Source,
    [string]$Name = 'macOS',
    # Must match prepare.ps1 -Cores on an AMD CPU (the patches bake it in).
    [int]$Cores = 8,
    [int]$RamGB = 8,
    # Enough to run builds made elsewhere; building inside it needs more.
    [int]$DiskGB = 80,
    # Defaults to where Hyper-V keeps its VMs.
    [string]$OutDir = (Get-VMHost).VirtualMachinePath
)
$ErrorActionPreference = 'Stop'
$vmDir = Join-Path $OutDir $Name

if (Get-VM -Name $Name -ErrorAction SilentlyContinue) { throw "A VM named '$Name' already exists." }
if ((Test-Path $vmDir) -and (Get-ChildItem $vmDir).Count) { throw "$vmDir already has files in it." }
foreach ($p in "$Source\efi\EFI\OC\config.plist", "$Source\recovery\com.apple.recovery.boot\BaseSystem.dmg") {
    if (-not (Test-Path $p)) { throw "Missing $p. Run prepare.ps1 first." }
}
New-Item -ItemType Directory -Force $vmDir | Out-Null

# ── EFI disk: OpenCore, the post-install tools, and the recovery image ──────
$efiVhd = Join-Path $vmDir 'EFI.vhdx'
Write-Host "Creating the EFI disk..."
$vol = New-VHD -Path $efiVhd -Dynamic -SizeBytes 5GB |
    Mount-VHD -Passthru |
    Initialize-Disk -PartitionStyle GPT -Confirm:$false -Passthru |
    New-Partition -AssignDriveLetter -UseMaximumSize |
    Format-Volume -FileSystem FAT32 -NewFileSystemLabel EFI -Confirm:$false -Force
$root = "$($vol.DriveLetter):"
Copy-Item "$Source\efi\EFI" -Recurse -Destination "$root\EFI"
New-Item -ItemType Directory -Force "$root\Scripts" | Out-Null
Get-ChildItem "$Source\efi\Scripts" -Filter *.sh | Copy-Item -Destination "$root\Scripts"
Copy-Item "$Source\efi\Tools" -Recurse -Destination "$root\Tools"
Copy-Item "$Source\recovery\com.apple.recovery.boot" -Recurse -Destination "$root\com.apple.recovery.boot"
Dismount-VHD -Path $efiVhd

# ── The VM ──────────────────────────────────────────────────────────────────
Write-Host "Creating the VM..."
New-VM -Generation 2 -Name $Name -Path $OutDir -NoVHD | Out-Null
Get-VMNetworkAdapter -VMName $Name | Connect-VMNetworkAdapter -SwitchName 'Default Switch'
Add-VMHardDiskDrive -VMName $Name -Path $efiVhd -ControllerType SCSI
$efiDisk = Get-VMHardDiskDrive -VMName $Name

# exFAT only so Windows can create it; the installer erases it to APFS.
$macVhd = Join-Path $vmDir "$Name.vhdx"
Write-Host "Creating the ${DiskGB} GB macOS disk..."
New-VHD -Path $macVhd -SizeBytes ($DiskGB * 1GB) |
    Mount-VHD -Passthru |
    Initialize-Disk -PartitionStyle GPT -Confirm:$false -Passthru |
    New-Partition -AssignDriveLetter -UseMaximumSize |
    Format-Volume -FileSystem exFAT -NewFileSystemLabel macOS -Confirm:$false -Force | Out-Null
Dismount-VHD -Path $macVhd
Add-VMHardDiskDrive -VMName $Name -Path $macVhd -ControllerType SCSI

Set-VM -Name $Name -ProcessorCount $Cores -MemoryStartupBytes ($RamGB * 1GB) `
    -StaticMemory -AutomaticCheckpointsEnabled $false
Set-VMFirmware -VMName $Name -EnableSecureBoot Off -FirstBootDevice $efiDisk

Write-Host ""
Write-Host "Created '$Name' in $vmDir ($Cores cores, $RamGB GB RAM, $DiskGB GB disk)." -ForegroundColor Green
Write-Host "Start it from Hyper-V Manager and open its console to install macOS."
